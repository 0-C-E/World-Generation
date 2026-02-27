//! Terrain classification and region labeling.
//!
//! After elevation is generated, every tile is classified as [`Water`],
//! [`Land`], or [`FarLand`] (decorative terrain beyond the playable area).
//! Connected land tiles are then grouped into numbered regions via flood-fill
//! so that each island gets a unique label.
//!
//! **All functions are parameterized** - nothing reads global constants. Pass
//! the values from [`WorldConfig`](crate::config::WorldConfig).

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
    /// Convert to the `u8` discriminant used in the binary format.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from a raw `u8` (as stored in chunk data) back to a variant.
    ///
    /// Unknown values default to [`Water`](Terrain::Water).
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
    water_threshold: f64,
    playable_radius: f64,
    farland_margin: f64,
) -> Vec<Vec<Terrain>> {
    let farland_radius = playable_radius + farland_margin;
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
// Water region detection
// ---------------------------------------------------------------------------

/// Return `true` if the water body containing `(x, y)` has at least
/// `min_size` tiles (used to distinguish oceans from puddles).
pub fn is_large_water_region(
    terrain: &[Vec<Terrain>],
    x: usize,
    y: usize,
    min_size: usize,
    map_size: usize,
) -> bool {
    if terrain[y][x] != Terrain::Water {
        return false;
    }

    let mut visited = std::collections::HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((x, y));

    while let Some((cx, cy)) = queue.pop_front() {
        if visited.len() >= min_size {
            return true;
        }
        if !visited.insert((cx, cy)) {
            continue;
        }
        for (nx, ny) in neighbors_4(cx, cy, map_size) {
            if terrain[ny][nx] == Terrain::Water && !visited.contains(&(nx, ny)) {
                queue.push_back((nx, ny));
            }
        }
    }

    visited.len() >= min_size
}

// ---------------------------------------------------------------------------
// Neighbor helpers
// ---------------------------------------------------------------------------

/// Return the 4-connected neighbors of `(x, y)` that lie within a
/// `map_size x map_size` grid (no heap allocation).
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
