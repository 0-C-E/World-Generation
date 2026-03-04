//! City slot placement and filtering.
//!
//! # Overview
//!
//! Cities must be placed on coastal Land tiles that meet strict criteria.
//! This module handles both discovery and validation.
//!
//! # Placement rules
//!
//! A tile qualifies as a city slot when:
//! 1. It is classified as [`Land`](crate::terrain::Terrain::Land)
//! 2. It lies within the playable radius
//! 3. It has sufficient land neighbors (default: 2)
//! 4. It has sufficient water neighbors (default: 2)
//! 5. At least one water neighbor belongs to a "large" water body (ocean, not puddle)
//! 6. It is far enough from all previously placed slots (minimum spacing grid)
//!
//! # Island filtering
//!
//! After placement, islands with too few city slots are discarded entirely.
//! This ensures every playable island has a minimum strategic value.
//! Cities on small islands are removed along with their island.
//!
//! # Example
//!
//! ```ignore
//! let slots = find_city_slots(&terrain, &water_bodies, &config);
//! let filtered = filter_city_slots_by_region(&slots, &region_labels, 6);
//! // `filtered` contains only cities on islands with ≥ 6 slots
//! ```

use std::collections::HashMap;

use crate::config::WorldConfig;
use crate::terrain::{Terrain, WaterBodies};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Discover all valid coastal city positions on the map.
///
/// # Algorithm
///
/// For each Land tile within the playable radius:
/// 1. Count land and water neighbors (4-connected adjacency)
/// 2. Collect water neighbor positions
/// 3. Check if minimum land/water neighbor counts are met
/// 4. Check if any water neighbor belongs to a "large" water body
/// 5. Check if the tile is far enough from all already-placed slots
///
/// If all checks pass, add the tile to the result and mark a spacing
/// radius around it as "taken" (no more cities within spacing distance).
///
/// # Parameters
///
/// - `spacing`: Minimum tile distance between cities (default: 5)
/// - `playable_radius`: Maximum distance from map center
/// - `min_land`, `min_water`: Neighbor count thresholds
/// - `min_body`: Minimum size for a water body to count as "large"
///
/// # Returns
///
/// Vector of `(x, y)` coordinates for valid city slots, unsorted.
pub fn find_city_slots(
    terrain: &[Vec<Terrain>],
    water: &WaterBodies,
    config: &WorldConfig,
) -> Vec<(usize, usize)> {
    let map_size = config.map_len();
    let spacing = config.city_spacing as usize;
    let radius = config.playable_radius as f64;
    let min_land = config.min_land_neighbors as usize;
    let min_water = config.min_water_neighbors as usize;
    let min_body = config.min_water_body_size as usize;
    let center = map_size / 2;

    let mut taken = vec![vec![false; map_size]; map_size];
    let mut slots = Vec::new();

    for y in spacing..(map_size - spacing) {
        for x in spacing..(map_size - spacing) {
            let dx = (x as isize - center as isize) as f64;
            let dy = (y as isize - center as isize) as f64;
            if (dx * dx + dy * dy).sqrt() > radius {
                continue;
            }
            if terrain[y][x] != Terrain::Land {
                continue;
            }

            let (land, water_count, water_positions) = count_neighbors(terrain, x, y, map_size);

            if land >= min_land
                && water_count >= min_water
                && !is_area_taken(&taken, x, y, spacing, map_size)
                && water_positions
                    .iter()
                    .any(|&(wx, wy)| water.is_large(wx, wy, min_body))
            {
                slots.push((x, y));
                mark_area_taken(&mut taken, x, y, spacing, map_size);
            }
        }
    }
    slots
}

/// Filter city slots by region, keeping only those on "large enough" islands.
///
/// # Algorithm
///
/// 1. Group city slots by their region ID (island)
/// 2. Filter groups to keep only those with ≥ `min_slots` slots
/// 3. Return all slots from surviving groups
///
/// # Purpose
///
/// Prevents tiny islands (with only 1–2 cities) from appearing in the game world.
/// These small islands either become unclaimed, or if claimed, remain strategically
/// weak with minimal resources. Filtering them simplifies the game.
///
/// # Parameters
///
/// - `min_slots`: Minimum cities required to keep an island (default: 6)
pub fn filter_city_slots_by_region(
    city_slots: &[(usize, usize)],
    region_map: &[Vec<usize>],
    min_slots: usize,
) -> Vec<(usize, usize)> {
    let mut by_region: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for &(x, y) in city_slots {
        let rid = region_map[y][x];
        if rid > 0 {
            by_region.entry(rid).or_default().push((x, y));
        }
    }

    by_region
        .into_values()
        .filter(|group| group.len() >= min_slots)
        .flatten()
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Count the land and water neighbours of `(x, y)` (4-connected).
fn count_neighbors(
    terrain: &[Vec<Terrain>],
    x: usize,
    y: usize,
    map_size: usize,
) -> (usize, usize, Vec<(usize, usize)>) {
    const DIRS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    let mut land = 0;
    let mut water = 0;
    let mut water_positions = Vec::new();

    for &(dx, dy) in &DIRS {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if nx >= map_size || ny >= map_size {
            continue;
        }
        match terrain[ny][nx] {
            Terrain::Land => land += 1,
            Terrain::Water => {
                water += 1;
                water_positions.push((nx, ny));
            }
            Terrain::FarLand => {}
        }
    }
    (land, water, water_positions)
}

/// Check whether any tile in the spacing box around `(x, y)` is already taken.
fn is_area_taken(taken: &[Vec<bool>], x: usize, y: usize, spacing: usize, map_size: usize) -> bool {
    let y0 = y.saturating_sub(spacing);
    let y1 = (y + spacing + 1).min(map_size);
    let x0 = x.saturating_sub(spacing);
    let x1 = (x + spacing + 1).min(map_size);

    for row in &taken[y0..y1] {
        for &cell in &row[x0..x1] {
            if cell {
                return true;
            }
        }
    }
    false
}

/// Mark the spacing box around `(x, y)` as taken.
fn mark_area_taken(taken: &mut [Vec<bool>], x: usize, y: usize, spacing: usize, map_size: usize) {
    let y0 = y.saturating_sub(spacing);
    let y1 = (y + spacing + 1).min(map_size);
    let x0 = x.saturating_sub(spacing);
    let x1 = (x + spacing + 1).min(map_size);

    for row in &mut taken[y0..y1] {
        for cell in &mut row[x0..x1] {
            *cell = true;
        }
    }
}
