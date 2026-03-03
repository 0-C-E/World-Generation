//! Map tile renderer.
//!
//! Produces 256 x 256 PNG tiles from chunk data, compatible with Leaflet
//! slippy-map tile URLs (`/tile/{z}/{x}/{y}.png`).
//!
//! Uses a two-phase approach that is borrow-checker friendly:
//! 1. Pre-load (`ensure_chunk`) all required chunks into the cache.
//! 2. Sample pixels via `chunk()` method with shared borrows once cached.
//!
//! Rendering modes:
//! - **Standard**: Biome colors with elevation shading (used by web viewer).
//! - **Debug**: Adds tile grid, coordinate labels, and gold vein overlay (for dev/debugging).

use crate::biome::{Biome, GoldVeinSampler};
use crate::font::draw_text;
use crate::terrain::Terrain;
use crate::world::World;

/// Side length of a tile image in pixels.
///
/// All tiles are square; this is both width and height.
pub const TILE_SIZE: u32 = 256;

/// The world-coordinate region that a single tile image covers.
///
/// Used to map pixel coordinates within a tile to world coordinates
/// for sampling elevation, terrain, and biome data.
struct TileRegion {
    /// World X coordinate of tile's left edge
    x_start: f64,
    /// World Y coordinate of tile's top edge
    y_start: f64,
    /// Width of tile region in world coordinates
    width: f64,
    /// Height of tile region in world coordinates
    height: f64,
}

/// Render a single map tile at the given zoom level and coordinates.
///
/// # Arguments
/// - `world`: Mutable reference to world (needed for chunk loading)
/// - `z`: Zoom level (higher = more zoomed in, more detail)
/// - `tx`, `ty`: Tile coordinates in the slippy-map system
///
/// # Returns
/// `Some(png_bytes)` if tile wordinate grid, boundaries, and gold vein overlay.
///
/// Includes visual aids for development:
/// - **Grid**: Tile boundaries and chunk edges
/// - **Coordinates**: X, Y position labels
/// - **Gold veins**: Thin yellow lines showing gold deposit locations
///
/// Debug tiles are not cached and are re-rendered on every request,
/// ensuring fresh visualization of any underlying data changesom level.
///
/// # Tile coordinates
/// At zoom level 0, the entire world is a single 256x256 tile.
/// At zoom level z, the world is divided into 2^z × 2^z tiles.
pub fn render_tile(world: &mut World, z: u32, tx: u32, ty: u32) -> Option<Vec<u8>> {
    let (pixels, _) = render_base(world, z, tx, ty)?;
    Some(encode_png(&pixels, TILE_SIZE, TILE_SIZE))
}

/// Render a debug tile with coordinate grid, boundaries, and gold vein overlay.
///
/// Includes visual aids for development:
/// - **Grid**: Tile boundaries and chunk edges
/// - **Coordinates**: X, Y position labels
/// - **Gold veins**: Thin yellow lines showing gold deposit locations
///
/// Debug tiles are not cached and are re-rendered on every request,
/// ensuring fresh visualization of any underlying data changes.
pub fn render_debug_tile(world: &mut World, z: u32, tx: u32, ty: u32) -> Option<Vec<u8>> {
    let seed = world.config().seed;
    let (mut pixels, region) = render_base(world, z, tx, ty)?;
    draw_gold_overlay(&mut pixels, world, &region, seed);
    draw_debug_overlays(&mut pixels, z, tx, ty, &region);
    Some(encode_png(&pixels, TILE_SIZE, TILE_SIZE))
}

// ---------------------------------------------------------------------------
// Shared rendering core
// ---------------------------------------------------------------------------

/// Shared rendering core - produces the base RGB pixel buffer for a tile.
///
/// Renders biome colors with elevation-based shading. This is the first phase
/// of rendering; output can then be modified with overlays (grid, coordinates, gold).
///
/// # Returns
/// `Some((pixels, region))` with the raw RGB buffer and the tile's world-coordinate region,
/// or `None` if coordinates are invalid for the zoom level.
fn render_base(world: &mut World, z: u32, tx: u32, ty: u32) -> Option<(Vec<u8>, TileRegion)> {
    let tiles_per_axis = 1u32 << z;
    let max_zoom = world.config().max_zoom();
    if tx >= tiles_per_axis || ty >= tiles_per_axis || z > max_zoom {
        return None;
    }

    // Copy scalars so we don't borrow `world` across the mutable chunk-loading phase.
    let width = world.width();
    let height = world.height();
    let chunk_size = world.config().chunk_size as u32;
    let water_threshold = world.config().water_threshold;
    let chunks_x = world.chunks_x();
    let chunks_y = world.chunks_y();

    let map_w = width as f64;
    let map_h = height as f64;
    let region = TileRegion {
        x_start: tx as f64 * map_w / tiles_per_axis as f64,
        y_start: ty as f64 * map_h / tiles_per_axis as f64,
        width: map_w / tiles_per_axis as f64,
        height: map_h / tiles_per_axis as f64,
    };

    // Determine which chunks overlap this tile.
    let cx_min = (region.x_start as u32) / chunk_size;
    let cx_max = (((region.x_start + region.width).ceil() as u32).min(width - 1) / chunk_size)
        .min(chunks_x - 1);
    let cy_min = (region.y_start as u32) / chunk_size;
    let cy_max = (((region.y_start + region.height).ceil() as u32).min(height - 1) / chunk_size)
        .min(chunks_y - 1);

    // Phase 1: load all needed chunks into the cache.
    for cy in cy_min..=cy_max {
        for cx in cx_min..=cx_max {
            if world.ensure_chunk(cx, cy).is_err() {
                return None;
            }
        }
    }

    // Phase 2: sample pixels from cached chunks.
    let mut pixels = vec![0u8; (TILE_SIZE * TILE_SIZE * 3) as usize];

    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let map_x = (region.x_start + px as f64 * region.width / TILE_SIZE as f64) as u32;
            let map_y = (region.y_start + py as f64 * region.height / TILE_SIZE as f64) as u32;
            let map_x = map_x.min(width - 1);
            let map_y = map_y.min(height - 1);

            let cx = map_x / chunk_size;
            let cy = map_y / chunk_size;

            if let Some(chunk) = world.chunk(cx, cy) {
                let lx = (map_x - cx * chunk_size) as usize;
                let ly = (map_y - cy * chunk_size) as usize;
                let idx = ly * chunk.width as usize + lx;

                let terrain = Terrain::from_u8(chunk.terrain[idx]);
                let biome = Biome::from_u8(chunk.biomes[idx]);
                let color = biome.get_color(terrain, chunk.elevation[idx], water_threshold);

                let off = ((py * TILE_SIZE + px) * 3) as usize;
                pixels[off] = color[0];
                pixels[off + 1] = color[1];
                pixels[off + 2] = color[2];
            }
        }
    }

    Some((pixels, region))
}

// ---------------------------------------------------------------------------
// Debug overlays
// ---------------------------------------------------------------------------

/// Draw diagnostic elements on top of a rendered tile.
fn draw_debug_overlays(pixels: &mut [u8], z: u32, tx: u32, ty: u32, region: &TileRegion) {
    // Border color cycles with zoom level.
    let border: [u8; 3] = match z % 4 {
        0 => [255, 0, 0],
        1 => [0, 255, 0],
        2 => [0, 100, 255],
        _ => [255, 255, 0],
    };

    // 2px border around the tile
    for i in 0..TILE_SIZE {
        for b in 0..2u32 {
            set_pixel(pixels, i, b, border);
            set_pixel(pixels, i, TILE_SIZE - 1 - b, border);
            set_pixel(pixels, b, i, border);
            set_pixel(pixels, TILE_SIZE - 1 - b, i, border);
        }
    }

    // Diagonal cross.
    for i in 0..TILE_SIZE {
        set_pixel(pixels, i, i, [255, 255, 255]);
        set_pixel(pixels, TILE_SIZE - 1 - i, i, [255, 255, 255]);
    }

    // Centre crosshair (10px arms).
    let mid = TILE_SIZE / 2;
    for d in 0..10 {
        if mid + d < TILE_SIZE {
            set_pixel(pixels, mid + d, mid, [255, 0, 255]);
            set_pixel(pixels, mid, mid + d, [255, 0, 255]);
        }
        if mid >= d {
            set_pixel(pixels, mid - d, mid, [255, 0, 255]);
            set_pixel(pixels, mid, mid - d, [255, 0, 255]);
        }
    }

    // Coordinate labels with drop shadow.
    let x_end = (region.x_start + region.width) as u32;
    let y_end = (region.y_start + region.height) as u32;

    let line1 = format!("z={z} x={tx} y={ty}");
    let line2 = format!("world: {}-{}", region.x_start as u32, region.y_start as u32);
    let line3 = format!("   to: {x_end}-{y_end}");
    let line4 = format!("region: {:.1}x{:.1}", region.width, region.height);

    let labels: [(u32, &str); 4] = [
        (5, &line1), (15, &line2), (25, &line3), (35, &line4),
    ];
    for (y, text) in labels {
        draw_text(pixels, TILE_SIZE, 6, y + 1, text, [0, 0, 0]);
        draw_text(pixels, TILE_SIZE, 5, y, text, [255, 255, 255]);
    }
}

// ---------------------------------------------------------------------------
// Gold overlay
// ---------------------------------------------------------------------------

/// Draw a bright marker on every tile that lies on a gold vein.
fn draw_gold_overlay(pixels: &mut [u8], world: &World, region: &TileRegion, seed: u32) {
    let width = world.width();
    let height = world.height();
    let chunk_size = world.config().chunk_size as u32;
    let sampler = GoldVeinSampler::new(seed);

    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let map_x = (region.x_start + px as f64 * region.width / TILE_SIZE as f64) as u32;
            let map_y = (region.y_start + py as f64 * region.height / TILE_SIZE as f64) as u32;
            let map_x = map_x.min(width - 1);
            let map_y = map_y.min(height - 1);

            let cx = map_x / chunk_size;
            let cy = map_y / chunk_size;

            if let Some(chunk) = world.chunk(cx, cy) {
                let lx = (map_x - cx * chunk_size) as usize;
                let ly = (map_y - cy * chunk_size) as usize;
                let idx = ly * chunk.width as usize + lx;

                let biome = Biome::from_u8(chunk.biomes[idx]);
                if sampler.is_gold(map_x as usize, map_y as usize, biome) {
                    // Gold vein pixel
                    let gold: [u8; 3] = [255, 215, 0];
                    set_pixel(pixels, px, py, gold);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pixel / PNG helpers
// ---------------------------------------------------------------------------

/// Write an RGB pixel into the tile buffer (bounds-checked).
///
/// Ensures writes don't go out of bounds. Silently ignores out-of-bounds
/// writes to handle edge cases gracefully.
fn set_pixel(pixels: &mut [u8], x: u32, y: u32, color: [u8; 3]) {
    let offset = ((y * TILE_SIZE + x) * 3) as usize;
    if offset + 2 < pixels.len() {
        pixels[offset] = color[0];
        pixels[offset + 1] = color[1];
        pixels[offset + 2] = color[2];
    }
}

/// Encode raw RGB pixels into a PNG image.
///
/// Returns the PNG byte buffer on success, or an empty buffer on encoding errors.
/// This graceful degradation ensures failed tiles don't crash the server.
fn encode_png(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut buffer = Vec::new();

    // Create and configure PNG encoder
    {
        let mut encoder = png::Encoder::new(&mut buffer, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);

        // Write PNG header
        let Ok(mut writer) = encoder.write_header() else {
            eprintln!("Warning: Failed to write PNG header for tile {}x{}", width, height);
            return Vec::new();
        };

        // Write image data
        if let Err(e) = writer.write_image_data(pixels) {
            eprintln!("Warning: Failed to write PNG image data: {}", e);
            return Vec::new();
        }
    }

    buffer
}
