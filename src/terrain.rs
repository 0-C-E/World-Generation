//! Terrain classification, region labeling, and distance field computation.
//!
//! # Overview
//!
//! This module converts a continuous elevation grid into discrete terrain categories
//! and discovers islands via flood-fill. It also computes ocean distance fields used
//! for village placement.
//!
//! # Terrain types
//!
//! Every tile is classified into one of three types:
//! - **Water**: Depth zones (elevation < threshold)
//! - **Land**: Playable area with cities and villages
//! - **FarLand**: Decorative border beyond the playable radius
//!
//! # Island discovery
//!
//! A standard flood-fill algorithm labels all connected `Land` tiles with unique
//! region IDs. Each region represents one island. The algorithm:
//! 1. Scans the map left-to-right, top-to-bottom
//! 2. When an unlabeled `Land` tile is found, fills the entire connected component
//! 3. Assigns a new region ID and continues
//!
//! Result: `region_labels[y][x]` gives the island ID (0 = non-land).
//!
//! # Water body detection
//!
//! Similar to island detection, but for water regions. Used to enforce minimum
//! water body size for city placement (ensures cities are on "real" oceans, not puddles).
//!
//! # Ocean distance field
//!
//! Computes distance from each tile to the nearest water or FarLand boundary.
//! Used by village placement to keep villages genuinely inland.

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Terrain enum
// ---------------------------------------------------------------------------

/// The three broad terrain categories partitioning the world.
///
/// These categories drive downstream logic (city placement, biome assignment, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Terrain {
    /// Ocean and lakes (not playable)
    Water = 0,
    /// Playable landmass (receives cities, villages, biomes)
    Land = 1,
    /// Decorative terrain beyond the playable area (not part of gameplay)
    FarLand = 2,
}

impl Terrain {
    /// Convert to the `u8` stored in the binary format.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from a `u8` read from chunk data.
    ///
    /// Unknown values fall back to [`Water`](Terrain::Water).
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Terrain::Land,
            2 => Terrain::FarLand,
            _ => Terrain::Water,
        }
    }
}

// ---------------------------------------------------------------------------
// Terrain classification
// ---------------------------------------------------------------------------

/// Assign terrain type to every tile based on elevation and distance from center.
///
/// # Classification rules
///
/// For each tile at `(x, y)`:
/// 1. If `elevation[y][x] < water_threshold` → [`Water`](Terrain::Water)
/// 2. Else if distance from center > `playable_radius + farland_margin` → [`FarLand`](Terrain::FarLand)
/// 3. Else → [`Land`](Terrain::Land)
///
/// The center is at `(map_size/2, map_size/2)`. Distance is Euclidean.
///
/// # Parameters
///
/// - `water_threshold`: Height above which tiles become land (typically 0.55)
/// - `playable_radius`: Maximum distance from center for `Land` tiles
/// - `farland_margin`: Gap between playable area and decorative border
pub fn classify_terrain(
    elevation: &[Vec<f64>],
    map_size: usize,
    water_threshold: f32,
    playable_radius: u16,
    farland_margin: u16,
) -> Vec<Vec<Terrain>> {
    let water_threshold = water_threshold as f64;
    let farland_radius = playable_radius as f64 + farland_margin as f64;
    let mut terrain = vec![vec![Terrain::Land; map_size]; map_size];
    let center = map_size / 2;

    for y in 0..map_size {
        for x in 0..map_size {
            let dx = (x as isize - center as isize) as f64;
            let dy = (y as isize - center as isize) as f64;
            let dist = (dx * dx + dy * dy).sqrt();

            terrain[y][x] = if elevation[y][x] < water_threshold {
                Terrain::Water
            } else if dist > farland_radius {
                Terrain::FarLand
            } else {
                Terrain::Land
            };
        }
    }
    terrain
}

// ---------------------------------------------------------------------------
// Region labeling
// ---------------------------------------------------------------------------

/// Flood-fill all connected [`Land`](Terrain::Land) tiles into numbered regions.
///
/// # Algorithm
///
/// Uses breadth-first search (BFS) via a queue:
/// 1. Scan all tiles
/// 2. When an unlabeled `Land` tile Found, initiate flood-fill
/// 3. BFS expands to all 4-connected neighbors
/// 4. All reached tiles get the same region ID
/// 5. Increment region ID and continue scanning
///
/// # Result
///
/// `labels[y][x]` contains:
/// - `0` if the tile is Water or FarLand
/// - Unique integer (1, 2, 3, ...) if the tile is Land
///
/// All Land tiles in the same connected component share the same label (island).
pub fn label_regions(terrain: &[Vec<Terrain>], map_size: usize) -> Vec<Vec<usize>> {
    let mut labels = vec![vec![0usize; map_size]; map_size];
    let mut current_label = 1;

    for y in 0..map_size {
        for x in 0..map_size {
            if terrain[y][x] == Terrain::Land && labels[y][x] == 0 {
                flood_fill(terrain, &mut labels, x, y, current_label, map_size);
                current_label += 1;
            }
        }
    }

    labels
}

/// BFS flood-fill starting at `(start_x, start_y)`.
fn flood_fill(
    terrain: &[Vec<Terrain>],
    labels: &mut [Vec<usize>],
    start_x: usize,
    start_y: usize,
    label: usize,
    map_size: usize,
) {
    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));
    labels[start_y][start_x] = label;

    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in neighbors_4(x, y, map_size) {
            if terrain[ny][nx] == Terrain::Land && labels[ny][nx] == 0 {
                labels[ny][nx] = label;
                queue.push_back((nx, ny));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Water body labeling
// ---------------------------------------------------------------------------

/// Pre-computed water body labels and their sizes.
///
/// Built once by [`label_water_bodies`] so that city placement can check
/// whether a water tile belongs to a large body in O(1) instead of
/// re-flooding the ocean for every candidate.
pub struct WaterBodies {
    /// Per-tile label (0 = not water, 1.. = water body id).
    pub labels: Vec<Vec<u32>>,
    /// Size of each body, indexed by label. Index 0 is unused.
    pub sizes: Vec<usize>,
}

impl WaterBodies {
    /// Check whether the water body at `(x, y)` has at least `min_size` tiles.
    pub fn is_large(&self, x: usize, y: usize, min_size: usize) -> bool {
        let label = self.labels[y][x] as usize;
        label > 0 && self.sizes[label] >= min_size
    }
}

/// Flood-fill label all connected [`Water`](Terrain::Water) tiles into
/// numbered bodies (1, 2, 3, ...) and record each body's tile count.
///
/// Runs once over the full map. Non-water tiles get label 0.
pub fn label_water_bodies(terrain: &[Vec<Terrain>], map_size: usize) -> WaterBodies {
    let mut labels = vec![vec![0u32; map_size]; map_size];
    let mut sizes = vec![0usize]; // index 0 unused
    let mut current_label = 1u32;

    for y in 0..map_size {
        for x in 0..map_size {
            if terrain[y][x] == Terrain::Water && labels[y][x] == 0 {
                let size = flood_fill_water(terrain, &mut labels, x, y, current_label, map_size);
                sizes.push(size);
                current_label += 1;
            }
        }
    }

    WaterBodies { labels, sizes }
}

/// BFS flood-fill for water, returns the number of tiles filled.
fn flood_fill_water(
    terrain: &[Vec<Terrain>],
    labels: &mut [Vec<u32>],
    start_x: usize,
    start_y: usize,
    label: u32,
    map_size: usize,
) -> usize {
    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));
    labels[start_y][start_x] = label;
    let mut count = 0;

    while let Some((x, y)) = queue.pop_front() {
        count += 1;
        for (nx, ny) in neighbors_4(x, y, map_size) {
            if terrain[ny][nx] == Terrain::Water && labels[ny][nx] == 0 {
                labels[ny][nx] = label;
                queue.push_back((nx, ny));
            }
        }
    }

    count
}

// ---------------------------------------------------------------------------
// Ocean distance map
// ---------------------------------------------------------------------------

/// Compute the minimum tile distance from each tile to the nearest
/// [`Water`](Terrain::Water) or [`FarLand`](Terrain::FarLand) tile.
///
/// Uses multi-source BFS seeded from every boundary tile simultaneously,
/// running in O(W × H) — identical complexity to a single flood-fill.
///
/// Results:
/// * Water / FarLand tiles → distance 0
/// * Land tiles → Manhattan BFS distance to nearest boundary tile
///
/// This is the foundation for inland village placement: a village at distance
/// `d` is guaranteed to have `d` Land tiles between it and the ocean.
/// Compute per-tile distance to nearest water or FarLand boundary.
///
/// # Purpose
///
/// Used by village placement to ensure villages are placed genuinely inland,
/// away from ocean edges. The distance field uses Chebyshev distance
/// (also called Chessboard distance: `max(|dx|, |dy|)`).
///
/// # Algorithm
///
/// Uses a multi-pass BFS:
/// 1. Seed the queue with all Water and FarLand tiles (distance = 0)
/// 2. Expand to neighbors, incrementing distance
/// 3. Each Land tile gets the shortest distance to any ocean/border tile
///
/// # Returns
///
/// A 2D grid where each Land tile contains its distance to nearest water boundary.
/// Water and FarLand tiles are set to 0 (already at the boundary).
pub fn compute_ocean_distances(terrain: &[Vec<Terrain>], map_size: usize) -> Vec<Vec<u32>> {
    let mut dist = vec![vec![u32::MAX; map_size]; map_size];
    let mut queue = VecDeque::with_capacity(map_size * 4);

    // Seed from all non-Land tiles simultaneously.
    for y in 0..map_size {
        for x in 0..map_size {
            if terrain[y][x] != Terrain::Land {
                dist[y][x] = 0;
                queue.push_back((x, y));
            }
        }
    }

    // Standard BFS — each tile is processed at most once.
    while let Some((x, y)) = queue.pop_front() {
        let d = dist[y][x] + 1;
        for (nx, ny) in neighbors_4(x, y, map_size) {
            if dist[ny][nx] == u32::MAX {
                dist[ny][nx] = d;
                queue.push_back((nx, ny));
            }
        }
    }

    dist
}

// ---------------------------------------------------------------------------
// Neighbor helpers
// ---------------------------------------------------------------------------

/// 4-connected neighbors of `(x, y)` inside a `map_size x map_size` grid.
pub fn neighbors_4(
    x: usize,
    y: usize,
    map_size: usize,
) -> impl Iterator<Item = (usize, usize)> {
    let mut buf = [(0usize, 0usize); 4];
    let mut len = 0;

    if x > 0             { buf[len] = (x - 1, y); len += 1; }
    if x + 1 < map_size  { buf[len] = (x + 1, y); len += 1; }
    if y > 0             { buf[len] = (x, y - 1); len += 1; }
    if y + 1 < map_size  { buf[len] = (x, y + 1); len += 1; }

    buf.into_iter().take(len)
}
