//! Per-city aggregated resource profiles.
//!
//! [`CityResources`] is computed once during world generation by scanning a
//! circular tile neighbourhood around each city slot and aggregating biome
//! modifiers. The result is written into the world file header so the game
//! and viewer can display resource previews at no runtime cost.

use std::collections::HashMap;

use rayon::prelude::*;

use crate::biome::{gold::GoldVeinSampler, Biome};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Scan radius in tiles around each city centre.
///
/// Cities sit on shores, so roughly half the scanned area is water
/// (contributing fishing food) and half is land. Radius 6 gives ~113 tiles —
/// a reasonable farmable hinterland for a single city.
pub const CITY_SCAN_RADIUS: i32 = 6;

// ---------------------------------------------------------------------------
// CityResources
// ---------------------------------------------------------------------------

/// Aggregated passive resource profile for a single city slot.
///
/// Stored in the world file, parallel to `city_slots`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CityResources {
    /// Aggregate passive wood modifier (average of tile bonuses in the scan radius).
    pub wood: i16,
    /// Aggregate passive stone modifier.
    pub stone: i16,
    /// Aggregate passive food modifier (includes fishing from water tiles).
    pub food: i16,
    /// Aggregate passive metal modifier.
    pub metal: i16,
    /// Aggregate passive Favor modifier (scaled by island size, see below).
    pub favor: i16,
    /// Number of gold-vein tiles within the scan radius.
    ///
    /// Gold is **never** passive — each node must be actively farmed.
    /// This count tells the player how many farmable sources are available.
    pub gold_nodes: u8,
    /// Dominant biome within the scan radius (for UI display).
    pub dominant_biome: u8,
}

// ---------------------------------------------------------------------------
// Computation
// ---------------------------------------------------------------------------

/// Compute [`CityResources`] for every city slot.
///
/// # Island-size Favor multiplier
///
/// Small islands (near `min_cities_per_island`) receive a large Favor bonus,
/// large islands almost none. This makes Sacred Grove + tiny island the
/// strongest Favor combination in the game.
///
/// ```text
/// ratio      = min_cities / island_cities   (1.0 for minimum-size islands)
/// multiplier = 1.0 + 2.0 × ratio²          (3× at minimum, ~1.1× for huge)
/// favor      = base_favor × multiplier
/// ```
///
/// Returns a `Vec` parallel to `city_slots`.
pub fn compute_city_resources(
    city_slots: &[(usize, usize)],
    biomes: &[Vec<u8>],
    region_labels: &[Vec<usize>],
    region_city_counts: &HashMap<usize, u32>,
    min_cities_per_island: u32,
    seed: u32,
) -> Vec<CityResources> {
    let map_h = biomes.len();
    let map_w = if map_h > 0 { biomes[0].len() } else { 0 };
    let r = CITY_SCAN_RADIUS;
    let min_f = min_cities_per_island.max(1) as f64;
    let gold_sampler = GoldVeinSampler::new(seed);

    city_slots
        .par_iter()
        .map(|&(cx, cy)| {
            let mut wood_sum: i32 = 0;
            let mut stone_sum: i32 = 0;
            let mut food_sum: i32 = 0;
            let mut metal_sum: i32 = 0;
            let mut favor_sum: i32 = 0;
            let mut gold_nodes: u8 = 0;
            let mut tile_count: u32 = 0;
            let mut biome_counts = [0u32; 16];

            for dy in -r..=r {
                for dx in -r..=r {
                    // Circular scan — skip corners.
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }

                    let tx = cx as i32 + dx;
                    let ty = cy as i32 + dy;
                    if tx < 0 || ty < 0 || tx >= map_w as i32 || ty >= map_h as i32 {
                        continue;
                    }
                    let (tx, ty) = (tx as usize, ty as usize);

                    let biome = Biome::from_u8(biomes[ty][tx]);
                    let mods = biome.resource_modifiers();
                    wood_sum += mods.wood as i32;
                    stone_sum += mods.stone as i32;
                    food_sum += mods.food as i32;
                    metal_sum += mods.metal as i32;
                    favor_sum += mods.favor as i32;
                    tile_count += 1;

                    let b = biome.to_u8() as usize;
                    if b < 16 {
                        biome_counts[b] += 1;
                    }

                    if gold_sampler.is_gold(tx, ty, biome) {
                        gold_nodes = gold_nodes.saturating_add(1);
                    }
                }
            }

            let n = tile_count.max(1) as i32;

            let dominant = biome_counts
                .iter()
                .enumerate()
                .max_by_key(|&(_, &c)| c)
                .map(|(i, _)| i as u8)
                .unwrap_or(0);

            // Island-size Favor multiplier.
            let region_id = region_labels[cy][cx];
            let island_cities = region_city_counts
                .get(&region_id)
                .copied()
                .unwrap_or(min_cities_per_island) as f64;
            let ratio = (min_f / island_cities).min(1.0);
            let favor_multiplier = 1.0 + 2.0 * ratio * ratio;
            let scaled_favor = ((favor_sum / n) as f64 * favor_multiplier).round();

            CityResources {
                wood: (wood_sum / n).clamp(-3200, 3200) as i16,
                stone: (stone_sum / n).clamp(-3200, 3200) as i16,
                food: (food_sum / n).clamp(-3200, 3200) as i16,
                metal: (metal_sum / n).clamp(-3200, 3200) as i16,
                favor: (scaled_favor as i32).clamp(-3200, 3200) as i16,
                gold_nodes,
                dominant_biome: dominant,
            }
        })
        .collect()
}
