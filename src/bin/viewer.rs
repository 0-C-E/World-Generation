//! Interactive map viewer - serves tiles and overlays over HTTP.
//!
//! Start with `cargo run --bin viewer [path]` and open `http://localhost:8080`.
//! Configuration is read from environment variables (and `.env` in dev).

use std::collections::HashMap;

use tiny_http::{Header, Request, Response, Server};

use world_generator::island::Island;
use world_generator::tile::{render_tile, render_debug_tile, TILE_SIZE};
use world_generator::World;

// ---------------------------------------------------------------------------
// Embedded assets
// ---------------------------------------------------------------------------

const HTML: &str = include_str!("../../static/index.html");
const CSS: &str = include_str!("../../static/style.css");
const JS: &str = include_str!("../../static/viewer.js");
const CITY_ICON_SVG: &str = include_str!("../../static/city-icon.svg");
const DEBUG_HTML: &str = include_str!("../../static/debug.html");
const DEBUG_CSS: &str = include_str!("../../static/debug.css");
const DEBUG_JS: &str = include_str!("../../static/debug.js");

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

struct ServerState {
    world: World,
    tile_cache: HashMap<(u32, u32, u32), Vec<u8>>,
    cities_json: Option<String>,
    islands_json: Option<String>,
    island_outlines: Option<HashMap<u32, String>>,
    world_fingerprint: String,
}

impl ServerState {
    fn ensure_cities_json(&mut self) {
        if self.cities_json.is_none() {
            self.cities_json = Some(build_cities_json(&mut self.world));
        }
    }

    fn ensure_islands_json(&mut self) {
        if self.islands_json.is_none() {
            self.world.ensure_islands_computed();
            self.islands_json = Some(islands_to_json(self.world.islands()));
        }
    }

    fn ensure_island_outlines(&mut self) {
        if self.island_outlines.is_none() {
            self.island_outlines = Some(build_island_outlines(&mut self.world));
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // Load .env file if present (silently ignored if missing).
    let _ = dotenvy::dotenv();

    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "world.world".into());

    if !std::path::Path::new(&path).exists() {
        eprintln!("File not found: {path}");
        std::process::exit(1);
    }

    // Fingerprint based on the world seed - changes when the world is regenerated.
    let world = World::open(&path).unwrap_or_else(|e| {
        eprintln!("Failed to open world: {e}");
        std::process::exit(1);
    });

    let fingerprint = format!("{:x}", world.config().seed);

    eprintln!(
        "Loaded {path}: {}x{} world, {} cities, seed {}",
        world.width(),
        world.height(),
        world.city_slots().len(),
        world.config().seed,
    );

    let mut state = ServerState {
        world,
        tile_cache: HashMap::new(),
        cities_json: None,
        islands_json: None,
        island_outlines: None,
        world_fingerprint: fingerprint,
    };

    // Eagerly compute islands and cities so the first request is instant.
    eprintln!("Pre-computing islands and cities...");
    state.ensure_islands_json();
    state.ensure_cities_json();
    eprintln!("Ready.");

    let addr = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let server = Server::http(&addr).expect("Failed to bind");
    eprintln!("Viewer running at http://{addr}");

    for request in server.incoming_requests() {
        let full_url = request.url().to_owned();
        // Strip query string so ?v=fingerprint doesn't affect routing.
        let url = full_url.split('?').next().unwrap_or(&full_url).to_owned();
        handle_request(request, &url, &mut state);
    }
}

// ---------------------------------------------------------------------------
// Request routing
// ---------------------------------------------------------------------------

fn handle_request(request: Request, url: &str, state: &mut ServerState) {
    match url {
        "/" => {
            // Inject the world fingerprint so tile URLs bust the cache on world change.
            let html = HTML.replace("{{WORLD_FINGERPRINT}}", &state.world_fingerprint);
            respond(request, "text/html; charset=utf-8", html);
        }
        "/style.css" => respond(request, "text/css", CSS),
        "/viewer.js" => {
            let js = inject_config(JS, &state);
            respond(request, "application/javascript", js);
        }
        "/debug" => {
            let html = DEBUG_HTML.replace("{{WORLD_FINGERPRINT}}", &state.world_fingerprint);
            respond(request, "text/html; charset=utf-8", html);
        }
        "/debug.css" => respond(request, "text/css", DEBUG_CSS),
        "/debug.js" => {
            let js = inject_config(DEBUG_JS, &state);
            respond(request, "application/javascript", js);
        }
        "/status" => respond(request, "application/json", r#"{"ready":true}"#),
        "/city-icon.svg" => respond(request, "image/svg+xml", CITY_ICON_SVG),
        "/cities.json" => {
            state.ensure_cities_json();
            respond(
                request,
                "application/json",
                state.cities_json.as_deref().unwrap(),
            );
        }

        "/islands.json" => {
            state.ensure_islands_json();
            respond(
                request,
                "application/json",
                state.islands_json.as_deref().unwrap(),
            );
        }

        _ if url.starts_with("/outline/") && url.ends_with(".json") => {
            handle_outline(request, url, state);
        }

        _ if url.starts_with("/dtile/") => {
            handle_debug_tile(request, url, state);
        }

        _ if url.starts_with("/tile/") => {
            handle_tile(request, url, state);
        }

        _ => {
            let _ = request.respond(
                Response::from_string("Not Found").with_status_code(404),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tile handlers
// ---------------------------------------------------------------------------

/// Parse `z/x/y.png` coordinates from a tile URL with the given prefix.
fn parse_tile_coords(url: &str, prefix: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = url.trim_start_matches(prefix).split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let z: u32 = parts[0].parse().ok()?;
    let x: u32 = parts[1].parse().ok()?;
    let y: u32 = parts[2].trim_end_matches(".png").parse().ok()?;
    Some((z, x, y))
}

fn handle_tile(request: Request, url: &str, state: &mut ServerState) {
    let Some((z, x, y)) = parse_tile_coords(url, "/tile/") else {
        let _ = request.respond(Response::from_string("Bad Request").with_status_code(400));
        return;
    };

    if !state.tile_cache.contains_key(&(z, x, y)) {
        match render_tile(&mut state.world, z, x, y) {
            Some(png) => { state.tile_cache.insert((z, x, y), png); }
            None => {
                let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
                return;
            }
        }
    }

    let png = state.tile_cache.get(&(z, x, y)).unwrap();
    let header = Header::from_bytes("Content-Type", "image/png").unwrap();
    let cache = Header::from_bytes("Cache-Control", format!("public, max-age={}", 60 * 15)).unwrap();
    let response = Response::from_data(png.clone()).with_header(header).with_header(cache);
    let _ = request.respond(response);
}

fn handle_debug_tile(request: Request, url: &str, state: &mut ServerState) {
    let Some((z, x, y)) = parse_tile_coords(url, "/dtile/") else {
        let _ = request.respond(Response::from_string("Bad Request").with_status_code(400));
        return;
    };

    match render_debug_tile(&mut state.world, z, x, y) {
        Some(png) => {
            let header = Header::from_bytes("Content-Type", "image/png").unwrap();
            let no_cache = Header::from_bytes("Cache-Control", "no-store").unwrap();
            let response = Response::from_data(png)
                .with_header(header)
                .with_header(no_cache);
            let _ = request.respond(response);
        }
        None => {
            let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
        }
    }
}

// ---------------------------------------------------------------------------
// Outline handler
// ---------------------------------------------------------------------------

fn handle_outline(request: Request, url: &str, state: &mut ServerState) {
    let id_str = url
        .trim_start_matches("/outline/")
        .trim_end_matches(".json");

    let rid: u32 = match id_str.parse() {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(
                Response::from_string("Bad Request").with_status_code(400),
            );
            return;
        }
    };

    state.ensure_island_outlines();
    let json = state
        .island_outlines
        .as_ref()
        .unwrap()
        .get(&rid)
        .map(String::as_str)
        .unwrap_or("[]");

    respond(request, "application/json", json);
}

// ---------------------------------------------------------------------------
// JSON builders
// ---------------------------------------------------------------------------

/// Build the JSON array for `/cities.json`.
///
/// Each entry is:
/// ```json
/// [x, y, region_label, {wood, stone, food, metal, favor, gold_nodes, biome}]
/// ```
fn build_cities_json(world: &mut World) -> String {
    let city_slots = world.city_slots().to_vec();
    let city_resources = world.city_resources().to_vec();
    let cs = world.config().chunk_size as u32;

    // Build an index-preserving list so we can match resources by position.
    let indexed: Vec<(usize, u32, u32)> = city_slots
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| (i, x, y))
        .collect();

    // Group cities by their containing chunk.
    let mut by_chunk: HashMap<(u32, u32), Vec<(usize, u32, u32)>> = HashMap::new();
    for &(i, x, y) in &indexed {
        by_chunk
            .entry((x / cs, y / cs))
            .or_default()
            .push((i, x, y));
    }

    let mut entries: Vec<(usize, String)> = Vec::with_capacity(city_slots.len());

    for ((cx, cy), cities) in &by_chunk {
        let _ = world.ensure_chunk(*cx, *cy);
        if let Some(chunk) = world.chunk(*cx, *cy) {
            let x0 = cx * cs;
            let y0 = cy * cs;
            for &(i, x, y) in cities {
                let lx = (x - x0) as usize;
                let ly = (y - y0) as usize;
                let idx = ly * chunk.width as usize + lx;
                let rid = chunk.region_labels[idx];
                let cr = city_resources.get(i).copied().unwrap_or_default();
                let biome_name = world_generator::biome::Biome::from_u8(cr.dominant_biome).name();
                entries.push((i, format!(
                    "[{x},{y},{rid},{{\"wood\":{},\"stone\":{},\"food\":{},\"metal\":{},\"favor\":{},\"gold_nodes\":{},\"biome\":\"{biome_name}\"}}]",
                    cr.wood, cr.stone, cr.food, cr.metal, cr.favor, cr.gold_nodes
                )));
            }
        }
    }

    // Sort by original index to keep stable ordering.
    entries.sort_by_key(|&(i, _)| i);
    let json_entries: Vec<&str> = entries.iter().map(|(_, s)| s.as_str()).collect();
    format!("[{}]", json_entries.join(","))
}

/// Serialize a list of islands to a JSON array.
///
/// Each island is `[id, centroid_x, centroid_y, city_count, min_x, min_y, max_x, max_y]`
/// to match what the Leaflet frontend expects.
fn islands_to_json(islands: &[Island]) -> String {
    let entries: Vec<String> = islands
        .iter()
        .map(|i| {
            format!(
                "[{},{},{},{},{},{},{},{}]",
                i.id,
                i.centroid.0,
                i.centroid.1,
                i.city_count,
                i.bounds.min_x,
                i.bounds.min_y,
                i.bounds.max_x,
                i.bounds.max_y,
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

// ---------------------------------------------------------------------------
// Island outline tracing
// ---------------------------------------------------------------------------

/// Build outline polylines for every island.
///
/// Requires all chunks to be loaded (triggered by `ensure_islands_computed`).
fn build_island_outlines(world: &mut World) -> HashMap<u32, String> {
    // Ensure all chunks and islands are loaded.
    world.ensure_islands_computed();

    let islands = world.islands().to_vec();
    let map_w = world.width();
    let map_h = world.height();

    let step = 4u32; // outline grid resolution (1 cell = `step` tiles)
    let mut outlines = HashMap::new();

    for island in &islands {
        let bb = &island.bounds;
        let pad = step * 2;
        let x0 = bb.min_x.saturating_sub(pad);
        let y0 = bb.min_y.saturating_sub(pad);
        let x1 = (bb.max_x + pad).min(map_w - 1);
        let y1 = (bb.max_y + pad).min(map_h - 1);

        let gw = ((x1 - x0) / step + 1) as usize;
        let gh = ((y1 - y0) / step + 1) as usize;

        if gw < 2 || gh < 2 {
            outlines.insert(island.id, "[]".to_owned());
            continue;
        }

        // Build a boolean grid: true = this island, false = not.
        let mut grid = vec![false; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let wx = x0 + gx as u32 * step;
                let wy = y0 + gy as u32 * step;
                if wx < map_w && wy < map_h {
                    grid[gy * gw + gx] =
                        world.region_label_at_cached(wx, wy) == island.id;
                }
            }
        }

        // Find boundary cells (island cells adjacent to non-island cells).
        let mut points = Vec::new();
        for gy in 0..gh {
            for gx in 0..gw {
                if !grid[gy * gw + gx] {
                    continue;
                }
                let is_edge = gx == 0
                    || gx + 1 == gw
                    || gy == 0
                    || gy + 1 == gh
                    || !grid[gy * gw + (gx - 1)]
                    || !grid[gy * gw + (gx + 1)]
                    || !grid[(gy - 1) * gw + gx]
                    || !grid[(gy + 1) * gw + gx];

                if is_edge {
                    let wx = x0 + gx as u32 * step;
                    let wy = y0 + gy as u32 * step;
                    points.push(format!("[{wx},{wy}]"));
                }
            }
        }

        outlines.insert(island.id, format!("[{}]", points.join(",")));
    }

    eprintln!("Built outlines for {} islands", outlines.len());
    outlines
}

// ---------------------------------------------------------------------------
// Config injection into JS templates
// ---------------------------------------------------------------------------

/// Replace `{{ MAP_SIZE }}`, `{{ TILE_SIZE }}`, and `{{ MAX_ZOOM }}` placeholders
/// in a JS template string with the actual world values.
fn inject_config(template: &str, state: &ServerState) -> String {
    let cfg = state.world.config();
    template
        .replace("{{ MAP_SIZE }}", &cfg.map_size.to_string())
        .replace("{{ TILE_SIZE }}", &TILE_SIZE.to_string())
        .replace("{{ MAX_ZOOM }}", &cfg.max_zoom().to_string())
}

// ---------------------------------------------------------------------------
// Response helper
// ---------------------------------------------------------------------------

fn respond(request: Request, content_type: &str, body: impl Into<String>) {
    let header = Header::from_bytes("Content-Type", content_type).unwrap();
    let response = Response::from_string(body).with_header(header);
    let _ = request.respond(response);
}
