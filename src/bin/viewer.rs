//! Interactive map viewer - serves tiles and overlays over HTTP.

use std::collections::HashMap;
use std::fmt::Write;
use std::time::Instant;

use tiny_http::{Header, Request, Response, Server};

use world_generator::island::Island;
use world_generator::tile::{render_debug_tile, render_tile, TILE_SIZE};
use world_generator::World;

// ---------------------------------------------------------------------------
// Embedded assets
// ---------------------------------------------------------------------------

const HTML: &str = include_str!("../../static/index.html");
const CSS: &str = include_str!("../../static/style.css");
const VIEWER_JS: &str = include_str!("../../static/viewer.js");
const NIGHT_MODE_JS: &str = include_str!("../../static/night-mode.js");
const CITY_ICON_SVG: &str = include_str!("../../static/city-icon.svg");
const VILLAGE_ICON_SVG: &str = include_str!("../../static/village-icon.svg");
const DEBUG_HTML: &str = include_str!("../../static/debug.html");
const DEBUG_CSS: &str = include_str!("../../static/debug.css");
const DEBUG_JS: &str = include_str!("../../static/debug.js");

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

struct ServerState {
    world: World,
    tile_cache: HashMap<(u32, u32, u32), Vec<u8>>,
    islands_json: Option<String>,
    island_outlines: Option<HashMap<u32, String>>,
    world_fingerprint: String,
}

impl ServerState {
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
    let _ = dotenvy::dotenv();

    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "world.world".into());

    if !std::path::Path::new(&path).exists() {
        eprintln!("File not found: {path}");
        std::process::exit(1);
    }

    let load_start = Instant::now();
    let world = World::open(&path).unwrap_or_else(|e| {
        eprintln!("Failed to open world: {e}");
        std::process::exit(1);
    });
    let load_elapsed = load_start.elapsed();

    let fingerprint = format!("{:x}", world.config().seed);

    eprintln!(
        "Loaded {path} in {load_elapsed:.2?}: {}x{} world, {} cities, {} villages, seed {}",
        world.width(),
        world.height(),
        world.city_slots().len(),
        world.villages().len(),
        world.config().seed,
    );

    let mut state = ServerState {
        world,
        tile_cache: HashMap::new(),
        islands_json: None,
        island_outlines: None,
        world_fingerprint: fingerprint,
    };

    eprintln!("Pre-computing islands...");
    let islands_start = Instant::now();
    state.ensure_islands_json();
    eprintln!("Islands ready in {:.2?}.", islands_start.elapsed());

    let addr = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let server = Server::http(&addr).expect("Failed to bind");
    eprintln!("Viewer running at http://{addr}");

    for request in server.incoming_requests() {
        let full_url = request.url().to_owned();
        let url = full_url.split('?').next().unwrap_or(&full_url).to_owned();
        handle_request(request, &url, &full_url, &mut state);
    }
}

// ---------------------------------------------------------------------------
// Request routing
// ---------------------------------------------------------------------------

fn handle_request(request: Request, url: &str, full_url: &str, state: &mut ServerState) {
    match url {
        "/" => {
            let html = HTML.replace("{{WORLD_FINGERPRINT}}", &state.world_fingerprint);
            respond(request, "text/html; charset=utf-8", html);
        }
        "/style.css" => respond(request, "text/css", CSS),
        "/viewer.js" => {
            let js = inject_config(VIEWER_JS, state);
            respond(request, "application/javascript", js);
        }
        "/night-mode.js" => {
            let js = inject_config(NIGHT_MODE_JS, state);
            respond(request, "application/javascript", js);
        }
        "/debug" => {
            let html = DEBUG_HTML.replace("{{WORLD_FINGERPRINT}}", &state.world_fingerprint);
            respond(request, "text/html; charset=utf-8", html);
        }
        "/debug.css" => respond(request, "text/css", DEBUG_CSS),
        "/debug.js" => {
            let js = inject_config(DEBUG_JS, state);
            respond(request, "application/javascript", js);
        }
        "/status" => respond(request, "application/json", r#"{"ready":true}"#),
        "/city-icon.svg" => respond(request, "image/svg+xml", CITY_ICON_SVG),
        "/village-icon.svg" => respond(request, "image/svg+xml", VILLAGE_ICON_SVG),

        "/cities" => handle_cities_viewport(request, full_url, state),
        "/villages" => handle_villages_viewport(request, full_url, state),
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
        _ if url.starts_with("/dtile/") => handle_debug_tile(request, url, state),
        _ if url.starts_with("/tile/") => handle_tile(request, url, state),
        _ => {
            let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
        }
    }
}

// ---------------------------------------------------------------------------
// Tile handlers
// ---------------------------------------------------------------------------

fn parse_tile_coords(url: &str, prefix: &str) -> Option<(u32, u32, u32)> {
    let mut parts = url.trim_start_matches(prefix).splitn(4, '/');
    let z: u32 = parts.next()?.parse().ok()?;
    let x: u32 = parts.next()?.parse().ok()?;
    let y: u32 = parts.next()?.trim_end_matches(".png").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((z, x, y))
}

fn handle_tile(request: Request, url: &str, state: &mut ServerState) {
    let Some((z, x, y)) = parse_tile_coords(url, "/tile/") else {
        let _ = request.respond(Response::from_string("Bad Request").with_status_code(400));
        return;
    };
    if !state.tile_cache.contains_key(&(z, x, y)) {
        match render_tile(&mut state.world, z, x, y) {
            Some(png) => {
                state.tile_cache.insert((z, x, y), png);
            }
            None => {
                let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
                return;
            }
        }
    }
    let png = state.tile_cache.get(&(z, x, y)).unwrap();
    let header = Header::from_bytes("Content-Type", "image/png").unwrap();
    let cache =
        Header::from_bytes("Cache-Control", format!("public, max-age={}", 60 * 15)).unwrap();
    let _ = request.respond(
        Response::from_data(png.clone())
            .with_header(header)
            .with_header(cache),
    );
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
            let _ = request.respond(
                Response::from_data(png)
                    .with_header(header)
                    .with_header(no_cache),
            );
        }
        None => {
            let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
        }
    }
}

// ---------------------------------------------------------------------------
// Viewport city handler
// ---------------------------------------------------------------------------

fn handle_cities_viewport(request: Request, full_url: &str, state: &mut ServerState) {
    let (x0, y0, x1, y1) = parse_bbox(full_url, state.world.width(), state.world.height());
    let json = build_cities_viewport_json(&mut state.world, x0, y0, x1, y1);
    let no_cache = Header::from_bytes("Cache-Control", "no-store").unwrap();
    let header = Header::from_bytes("Content-Type", "application/json").unwrap();
    let _ = request.respond(
        Response::from_string(json)
            .with_header(header)
            .with_header(no_cache),
    );
}

fn build_cities_viewport_json(world: &mut World, x0: u32, y0: u32, x1: u32, y1: u32) -> String {
    let city_slots = world.city_slots().to_vec();
    let city_resources = world.city_resources().to_vec();
    let cs = world.config().chunk_size as u32;

    let in_bbox: Vec<(usize, u32, u32)> = city_slots
        .iter()
        .enumerate()
        .filter(|(_, &(x, y))| x >= x0 && x <= x1 && y >= y0 && y <= y1)
        .map(|(i, &(x, y))| (i, x, y))
        .collect();

    type ChunkEntry = Vec<(usize, u32, u32)>;
    let mut by_chunk: HashMap<(u32, u32), ChunkEntry> = HashMap::new();
    for &(i, x, y) in &in_bbox {
        by_chunk
            .entry((x / cs, y / cs))
            .or_default()
            .push((i, x, y));
    }

    let mut entries: Vec<(usize, String)> = Vec::with_capacity(in_bbox.len());

    for ((cx, cy), cities) in &by_chunk {
        let _ = world.ensure_chunk(*cx, *cy);
        if let Some(chunk) = world.chunk(*cx, *cy) {
            let ox = cx * cs;
            let oy = cy * cs;
            for &(i, x, y) in cities {
                let lx = (x - ox) as usize;
                let ly = (y - oy) as usize;
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

    entries.sort_by_key(|&(i, _)| i);
    let parts: Vec<&str> = entries.iter().map(|(_, s)| s.as_str()).collect();
    format!("[{}]", parts.join(","))
}

// ---------------------------------------------------------------------------
// Viewport village handler
// ---------------------------------------------------------------------------

/// Serve villages within the requested world-coordinate bbox.
///
/// Query params: `x0`, `y0`, `x1`, `y1` (world tile coordinates).
///
/// Returns a JSON array:
/// `[[x, y, region_id, offers_name, demands_name, biome_name], ...]`
fn handle_villages_viewport(request: Request, full_url: &str, state: &mut ServerState) {
    let (x0, y0, x1, y1) = parse_bbox(full_url, state.world.width(), state.world.height());
    let json = build_villages_viewport_json(&state.world, x0, y0, x1, y1);
    let no_cache = Header::from_bytes("Cache-Control", "no-store").unwrap();
    let header = Header::from_bytes("Content-Type", "application/json").unwrap();
    let _ = request.respond(
        Response::from_string(json)
            .with_header(header)
            .with_header(no_cache),
    );
}

fn build_villages_viewport_json(world: &World, x0: u32, y0: u32, x1: u32, y1: u32) -> String {
    let villages: Vec<_> = world
        .villages()
        .iter()
        .filter(|v| {
            let vx = v.x as u32;
            let vy = v.y as u32;
            vx >= x0 && vx <= x1 && vy >= y0 && vy <= y1
        })
        .collect();

    let mut out = String::with_capacity(villages.len() * 48);
    out.push('[');
    let mut first = true;

    for v in villages {
        let biome_name = world_generator::biome::Biome::from_u8(v.biome).name();
        if !first {
            out.push(',');
        }
        first = false;
        use std::fmt::Write;
        let _ = write!(
            out,
            "[{},{},{},\"{}\",\"{}\",\"{}\"]",
            v.x,
            v.y,
            v.region_id,
            v.trade.offers.name(),
            v.trade.demands.name(),
            biome_name,
        );
    }

    out.push(']');
    out
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
            let _ = request.respond(Response::from_string("Bad Request").with_status_code(400));
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

fn islands_to_json(islands: &[Island]) -> String {
    let mut out = String::with_capacity(islands.len() * 64);
    out.push('[');
    use std::fmt::Write;
    for (i, island) in islands.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "[{},{},{},{},{},{},{},{},{},{}]",
            island.id,
            island.centroid.0,
            island.centroid.1,
            island.city_count,
            island.bounds.min_x,
            island.bounds.min_y,
            island.bounds.max_x,
            island.bounds.max_y,
            island.is_world_spawn as u8,
            island.spawn_order,
        );
    }
    out.push(']');
    out
}

// ---------------------------------------------------------------------------
// Island outline tracing
// ---------------------------------------------------------------------------

fn build_island_outlines(world: &mut World) -> HashMap<u32, String> {
    world.ensure_islands_computed();
    let islands = world.islands().to_vec();
    let map_w = world.width();
    let map_h = world.height();
    let step = 4u32;
    let mut outlines = HashMap::with_capacity(islands.len());

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

        let mut grid = vec![false; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let wx = x0 + gx as u32 * step;
                let wy = y0 + gy as u32 * step;
                if wx < map_w && wy < map_h {
                    grid[gy * gw + gx] = world.region_label_at_cached(wx, wy) == island.id;
                }
            }
        }

        let mut out = String::with_capacity((gw + gh) * 2 * 16);
        out.push('[');
        let mut first = true;
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
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    let _ = write!(out, "[{wx},{wy}]");
                }
            }
        }
        out.push(']');
        outlines.insert(island.id, out);
    }

    eprintln!("Built outlines for {} islands", outlines.len());
    outlines
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parse `x0`, `y0`, `x1`, `y1` query params, clamped to world bounds.
fn parse_bbox(full_url: &str, world_w: u32, world_h: u32) -> (u32, u32, u32, u32) {
    let query = full_url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut x0 = 0u32;
    let mut y0 = 0u32;
    let mut x1 = world_w.saturating_sub(1);
    let mut y1 = world_h.saturating_sub(1);
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("");
        let val: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "x0" => x0 = val.min(world_w),
            "y0" => y0 = val.min(world_h),
            "x1" => x1 = val.min(world_w.saturating_sub(1)),
            "y1" => y1 = val.min(world_h.saturating_sub(1)),
            _ => {}
        }
    }
    (x0, y0, x1, y1)
}

fn inject_config(template: &str, state: &ServerState) -> String {
    let cfg = state.world.config();
    template
        .replace("{{ MAP_SIZE }}", &cfg.map_size.to_string())
        .replace("{{ TILE_SIZE }}", &TILE_SIZE.to_string())
        .replace("{{ MAX_ZOOM }}", &cfg.max_zoom().to_string())
}

fn respond(request: Request, content_type: &str, body: impl Into<String>) {
    let header = Header::from_bytes("Content-Type", content_type).unwrap();
    let response = Response::from_string(body).with_header(header);
    let _ = request.respond(response);
}
