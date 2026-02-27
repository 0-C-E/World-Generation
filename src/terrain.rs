//! Terrain classification and region labeling.
//!
//! After elevation is generated, every tile is classified as [`Water`],
//! [`Land`], or [`FarLand`] (decorative terrain beyond the playable area).
//! Connected land tiles are then grouped into numbered regions via flood-fill
//! so that each island gets a unique label.

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Terrain enum
// ---------------------------------------------------------------------------

/// The three broad terrain categories for each tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Terrain {
    Water = 0,
    Land = 1,
    /// Decorative land beyond the playable radius.
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

/// Assign a [`Terrain`] category to every tile.
///
/// * Elevation below `water_threshold` -> [`Water`](Terrain::Water)
/// * Distance from center > `playable_radius + farland_margin` -> [`FarLand`](Terrain::FarLand)
/// * Everything else -> [`Land`](Terrain::Land)
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

/// Flood-fill label all connected [`Land`] tiles into numbered regions
/// (1, 2, 3, ...). Water and FarLand tiles keep label 0.
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

    if x > 0 {
        buf[len] = (x - 1, y);
        len += 1;
    }
    if x + 1 < map_size {
        buf[len] = (x + 1, y);
        len += 1;
    }
    if y > 0 {
        buf[len] = (x, y - 1);
        len += 1;
    }
    if y + 1 < map_size {
        buf[len] = (x, y + 1);
        len += 1;
    }

    buf.into_iter().take(len)
}
