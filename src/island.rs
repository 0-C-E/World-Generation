//! Island discovery and representation module.
//!
//! An island is a contiguous region of land tiles (see [`Land`](crate::terrain::Terrain::Land))
//! that contains at least one city. This module provides the [`Island`] type and
//! a discovery function to build the island registry from chunked world data.

use std::collections::HashMap;

use crate::save::{ChunkData, ChunkedWorldReader};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Represents an axis-aligned bounding box in world coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl BoundingBox {
    /// Creates a bounding box containing a single point.
    pub fn point(x: u32, y: u32) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    /// Expands the bounding box to include the point `(x, y)`.
    pub fn expand(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

/// Represents a named landmass containing one or more cities.
///
/// Each island is a distinct strategic location that players can colonize and compete over.
#[derive(Debug, Clone)]
pub struct Island {
    /// Unique region label from flood-fill (one per landmass).
    pub id: u32,
    /// Number of city slots on this island.
    pub city_count: u32,
    /// Average position (centroid) of all cities on the island.
    pub centroid: (u32, u32),
    /// Axis-aligned bounding box around the island's tiles.
    pub bounds: BoundingBox,
    /// True if this island is the designated world spawn (largest island).
    pub is_world_spawn: bool,
    /// Population order: 0 = world spawn, 1 = nearest to spawn, 2 = next, ...
    ///
    /// Determined by straight-line centroid distance from the world spawn
    /// island. Islands are filled in order from closest to furthest so that
    /// early-game expansion fans outward naturally from the spawn point.
    pub spawn_order: u32,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Scans city slots and chunk data to build the island registry.
///
/// After assembling basic island data, this function:
/// 1. Tags the island with the most city slots as the world spawn.
/// 2. Ranks every other island by centroid distance from the spawn, assigning `spawn_order` values starting at 1.
pub fn discover_islands(
    reader: &ChunkedWorldReader,
    chunk_cache: &mut HashMap<(u32, u32), ChunkData>,
) -> Vec<Island> {
    let header = &reader.header;
    let chunk_size = header.config.chunk_size as u32;

    // Step 1: Gather per-region city stats (sum_x, sum_y, count).
    let mut city_stats: HashMap<u32, (u64, u64, u32)> = HashMap::new();
    for &(x, y) in &header.city_slots {
        let region_id = region_label_at(reader, chunk_cache, chunk_size, x, y);
        if region_id == 0 {
            continue;
        }
        let entry = city_stats.entry(region_id).or_insert((0, 0, 0));
        entry.0 += x as u64;
        entry.1 += y as u64;
        entry.2 += 1;
    }

    // Step 2: Load every chunk and compute bounding boxes from region tiles.
    let mut bounding_boxes: HashMap<u32, BoundingBox> = HashMap::new();
    for cy in 0..header.chunks_y {
        for cx in 0..header.chunks_x {
            ensure_chunk(reader, chunk_cache, cx, cy);
            let Some(chunk) = chunk_cache.get(&(cx, cy)) else {
                continue;
            };
            let x0 = cx * chunk_size;
            let y0 = cy * chunk_size;
            for ly in 0..chunk.height {
                for lx in 0..chunk.width {
                    let region_id = chunk.region_labels[(ly * chunk.width + lx) as usize];
                    if region_id == 0 || !city_stats.contains_key(&region_id) {
                        continue;
                    }
                    bounding_boxes
                        .entry(region_id)
                        .and_modify(|bb| bb.expand(x0 + lx, y0 + ly))
                        .or_insert_with(|| BoundingBox::point(x0 + lx, y0 + ly));
                }
            }
        }
    }

    // Step 3: Assemble Island structs (is_world_spawn and spawn_order filled below).
    let mut islands: Vec<Island> = city_stats
        .into_iter()
        .map(|(region_id, (sum_x, sum_y, count))| Island {
            id: region_id,
            city_count: count,
            centroid: ((sum_x / count as u64) as u32, (sum_y / count as u64) as u32),
            bounds: bounding_boxes.get(&region_id).copied().unwrap_or_default(),
            is_world_spawn: false,
            spawn_order: 0,
        })
        .collect();

    // Step 4: Tag the largest island as world spawn.
    let spawn_centroid = if let Some(spawn) = islands.iter_mut().max_by_key(|i| i.city_count) {
        spawn.is_world_spawn = true;
        eprintln!(
            "Tagged island {} as world spawn ({} cities)",
            spawn.id, spawn.city_count
        );
        spawn.centroid
    } else {
        // No islands at all -- nothing more to do.
        return islands;
    };

    // Step 5: Rank remaining islands by distance from the spawn centroid.
    // Use squared distance to avoid sqrt; relative order is identical.
    let (sx, sy) = (spawn_centroid.0 as i64, spawn_centroid.1 as i64);
    let mut non_spawn: Vec<&mut Island> =
        islands.iter_mut().filter(|i| !i.is_world_spawn).collect();

    non_spawn.sort_by_key(|island| {
        let dx = island.centroid.0 as i64 - sx;
        let dy = island.centroid.1 as i64 - sy;
        dx * dx + dy * dy
    });

    for (order, island) in non_spawn.iter_mut().enumerate() {
        island.spawn_order = (order + 1) as u32;
    }

    islands.sort_by_key(|i| i.id);
    eprintln!("Discovered {} islands", islands.len());
    islands
}

// ---------------------------------------------------------------------------
// Chunk helpers (also used by world.rs)
// ---------------------------------------------------------------------------

/// Loads a chunk into the cache if it isn't already present.
pub(crate) fn ensure_chunk(
    reader: &ChunkedWorldReader,
    cache: &mut HashMap<(u32, u32), ChunkData>,
    cx: u32,
    cy: u32,
) {
    use std::collections::hash_map::Entry;
    if let Entry::Vacant(e) = cache.entry((cx, cy)) {
        if let Ok(chunk) = reader.load_chunk(cx, cy) {
            e.insert(chunk);
        }
    }
}

/// Returns the region label for a world coordinate, loading the containing chunk if necessary.
fn region_label_at(
    reader: &ChunkedWorldReader,
    cache: &mut HashMap<(u32, u32), ChunkData>,
    chunk_size: u32,
    x: u32,
    y: u32,
) -> u32 {
    let (chunk_x, chunk_y) = (x / chunk_size, y / chunk_size);
    ensure_chunk(reader, cache, chunk_x, chunk_y);
    cache
        .get(&(chunk_x, chunk_y))
        .map(|chunk| {
            let local_x = (x - chunk_x * chunk_size) as usize;
            let local_y = (y - chunk_y * chunk_size) as usize;
            chunk.region_labels[local_y * chunk.width as usize + local_x]
        })
        .unwrap_or(0)
}
