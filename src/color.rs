//! Biome / elevation to colour mapping for the tile renderer.
//!
//! Each tile is coloured primarily by its [`Biome`] classification, with
//! elevation providing subtle shading variation. Water tiles that lack
//! biome data fall back to depth-based blue gradients.

use crate::biome::Biome;
use crate::terrain::Terrain;

/// Map a terrain type, biome, and elevation to an RGB colour triple.
///
/// `elevation` is in `[0.0, 1.0]`.
pub fn get_color(terrain: Terrain, elevation: f32, water_threshold: f32, biome: Biome) -> [u8; 3] {
    let e = elevation;

    match terrain {
        Terrain::Water => water_color(e, water_threshold, biome),
        Terrain::Land => land_color(e, biome),
        Terrain::FarLand => farland_color(e),
    }
}

/// Water colouring — biome-aware (DeepHarbor / Coast / Ocean).
fn water_color(e: f32, water_threshold: f32, biome: Biome) -> [u8; 3] {
    match biome {
        Biome::DeepHarbor => {
            // Very dark blue — deep navigable waters
            let t = (e / water_threshold).clamp(0.0, 1.0);
            let r = (10.0 + t * 15.0) as u8;
            let g = (25.0 + t * 45.0) as u8;
            let b = (70.0 + t * 50.0) as u8;
            [r, g, b]
        }
        Biome::Coast => {
            // Lighter cyan — shallow coastal water, smoother transition
            let t = ((e - 0.18) / (water_threshold - 0.18)).clamp(0.0, 1.0).powf(0.7);
            let r = (40.0 + t * 70.0) as u8;
            let g = (110.0 + t * 90.0) as u8;
            let b = (150.0 + t * 60.0) as u8;
            [r, g, b]
        }
        _ => {
            // Standard ocean gradient: deep blue → lighter cyan, more gradual
            let t = ((e - 0.10) / (water_threshold - 0.10)).clamp(0.0, 1.0).powf(0.7);
            let r = ((1.0 - t) * 10.0 + t * 80.0) as u8;
            let g = ((1.0 - t) * 35.0 + t * (170.0 + e * 60.0)) as u8;
            let b = ((1.0 - t) * 80.0 + t * (200.0 + e * 60.0)) as u8;
            [r, g, b]
        }
    }
}

/// Land coloring -- driven by biome with elevation shading.
///
/// Every biome uses an elevation-based gradient so the map shows visible
/// terrain relief. Land tiles sit above the water threshold (~0.55), so raw
/// elevation is rescaled to [0, 1] over the land range for much stronger
/// visual contrast.
fn land_color(e: f32, biome: Biome) -> [u8; 3] {
    let land = ((e - 0.5) / 0.5).clamp(0.0, 1.0);

    match biome {
        Biome::Beach => {
            // Warm sand -- wet near water, dry and pale further up
            let t = land;
            let r = (210.0 + t * 25.0) as u8;
            let g = (195.0 + t * 25.0) as u8;
            let b = (150.0 + t * 35.0) as u8;
            [r, g, b]
        }

        Biome::Plains => {
            // Light green fields with visible elevation relief
            let t = land;
            let r = (75.0 + t * 80.0) as u8;
            let g = (130.0 + t * 65.0) as u8;
            let b = (30.0 + t * 60.0) as u8;
            [r, g, b]
        }

        Biome::Forest => {
            // Dense dark green -- sinusoidal texture + elevation gradient
            let t = land;
            let v = (12.0 * (e * 30.0).sin()) as i32;
            let r = (20.0 + t * 40.0) as u8;
            let g = ((80.0 + t * 45.0) as i32 + v).clamp(55, 145) as u8;
            let b = (15.0 + t * 30.0) as u8;
            [r, g, b]
        }

        Biome::Swamp => {
            // Murky dark olive-green
            let t = land;
            let r = (45.0 + t * 40.0) as u8;
            let g = (65.0 + t * 40.0) as u8;
            let b = (35.0 + t * 30.0) as u8;
            [r, g, b]
        }

        Biome::Hills => {
            // Yellow-green to rocky gray
            let t = land;
            let r = (100.0 + t * 55.0) as u8;
            let g = (110.0 + t * 40.0) as u8;
            let b = (60.0 + t * 45.0) as u8;
            [r, g, b]
        }

        Biome::Mountains => {
            // Gray-brown rock
            let t = land;
            let r = (115.0 + t * 50.0) as u8;
            let g = (105.0 + t * 45.0) as u8;
            let b = (90.0 + t * 45.0) as u8;
            [r, g, b]
        }

        Biome::SnowyPeaks => {
            // Blue-white snow
            let t = land;
            let r = (170.0 + t * 70.0) as u8;
            let g = (175.0 + t * 65.0) as u8;
            let b = (190.0 + t * 65.0) as u8;
            [r, g, b]
        }

        Biome::Desert => {
            // Sandy yellow / tan -- dunes with visible shading
            let t = land;
            let r = (185.0 + t * 40.0) as u8;
            let g = (160.0 + t * 40.0) as u8;
            let b = (95.0 + t * 45.0) as u8;
            [r, g, b]
        }

        Biome::Tundra => {
            // Icy tundra -- dark blue-grey in valleys, pale frost on ridges
            let t = land;
            let r = (95.0 + t * 100.0) as u8;
            let g = (110.0 + t * 90.0) as u8;
            let b = (135.0 + t * 75.0) as u8;
            [r, g, b]
        }

        Biome::Valley => {
            // Lush bright green (fertile low ground)
            let t = land;
            let r = (55.0 + t * 55.0) as u8;
            let g = (130.0 + t * 45.0) as u8;
            let b = (35.0 + t * 40.0) as u8;
            [r, g, b]
        }

        Biome::Highlands => {
            // Stony brown-grey plateau
            let t = land;
            let r = (100.0 + t * 75.0) as u8;
            let g = (90.0 + t * 70.0) as u8;
            let b = (70.0 + t * 70.0) as u8;
            [r, g, b]
        }

        Biome::SacredGrove => {
            // Mystical teal-green (ancient canopy)
            let t = land;
            let r = (15.0 + t * 65.0) as u8;
            let g = (85.0 + t * 70.0) as u8;
            let b = (50.0 + t * 60.0) as u8;
            [r, g, b]
        }

        // Fallback: generic elevation-based green
        _ => {
            let t = land;
            let r = (90.0 + t * 60.0) as u8;
            let g = (140.0 + t * 40.0) as u8;
            let b = (50.0 + t * 40.0) as u8;
            [r, g, b]
        }
    }
}

/// Washed-out beige beyond the playable area.
fn farland_color(e: f32) -> [u8; 3] {
    let t = (e - 0.8) / 0.2;
    let r = (200.0 + t * 55.0) as u8;
    let g = (180.0 + t * 75.0) as u8;
    let b = (160.0 + t * 95.0) as u8;
    [r, g, b]
}
