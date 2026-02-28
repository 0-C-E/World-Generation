//! Island discovery and representation.
//!
//! An island is a connected region of [`Land`](crate::terrain::Terrain::Land)
//! tiles that contains at least one city. This module provides the [`Island`]
//! type and a discovery function that scans chunk data to build the island
//! registry.

use std::collections::HashMap;

use crate::save::{ChunkData, ChunkedWorldReader};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An axis-aligned bounding box in world coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl BoundingBox {
    /// Create a bounding box containing a single point.
    pub fn point(x: u32, y: u32) -> Self {
        Self { min_x: x, min_y: y, max_x: x, max_y: y }
    }

    /// Expand the box to include `(x, y)`.
    pub fn expand(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

/// A named landmass containing one or more cities.
///
/// In a Grepolis-style game each island is a distinct strategic location that
/// players colonize and compete over.
#[derive(Debug, Clone)]
pub struct Island {
    /// Region label from the flood-fill (unique per landmass).
    pub id: u32,
    /// Number of city slots on this island.
    pub city_count: u32,
    /// Average position of all cities on the island.
    pub centroid: (u32, u32),
    /// Tight axis-aligned bounding box around the island's tiles.
    pub bounds: BoundingBox,
    /// Whether this island is the designated world spawn (largest island).
    /// New players place their first city here.
    pub world_spawn: bool,
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

/// Scan city slots and chunk data to build the island registry.
///
/// After assembling basic island data the function:
/// 1. Tags the island with the most city slots as the world spawn.
/// 2. Ranks every other island by centroid distance from the spawn,
///    assigning `spawn_order` values starting at 1.
pub fn discover_islands(
    reader: &ChunkedWorldReader,
    chunk_cache: &mut HashMap<(u32, u32), ChunkData>,
) -> Vec<Island> {
    let h = &reader.header;
    let cs = h.config.chunk_size as u32;

    // Step 1: accumulate per-region city stats (sum_x, sum_y, count).
    let mut stats: HashMap<u32, (u64, u64, u32)> = HashMap::new();
    for &(x, y) in &h.city_slots {
        let rid = region_label_at(reader, chunk_cache, cs, x, y);
        if rid == 0 {
            continue;
        }
        let e = stats.entry(rid).or_insert((0, 0, 0));
        e.0 += x as u64;
        e.1 += y as u64;
        e.2 += 1;
    }

    // Step 2: load every chunk and compute bounding boxes.
    let mut bboxes: HashMap<u32, BoundingBox> = HashMap::new();
    for cy in 0..h.chunks_y {
        for cx in 0..h.chunks_x {
            ensure_chunk(reader, chunk_cache, cx, cy);
            let Some(chunk) = chunk_cache.get(&(cx, cy)) else {
                continue;
            };
            let x0 = cx * cs;
            let y0 = cy * cs;
            for ly in 0..chunk.height {
                for lx in 0..chunk.width {
                    let rid = chunk.region_labels[(ly * chunk.width + lx) as usize];
                    if rid == 0 || !stats.contains_key(&rid) {
                        continue;
                    }
                    bboxes
                        .entry(rid)
                        .and_modify(|bb| bb.expand(x0 + lx, y0 + ly))
                        .or_insert_with(|| BoundingBox::point(x0 + lx, y0 + ly));
                }
            }
        }
    }

    // Step 3: assemble Island structs (world_spawn / spawn_order filled below).
    let mut islands: Vec<Island> = stats
        .into_iter()
        .map(|(rid, (sx, sy, count))| Island {
            id: rid,
            city_count: count,
            centroid: (
                (sx / count as u64) as u32,
                (sy / count as u64) as u32,
            ),
            bounds: bboxes.get(&rid).copied().unwrap_or_default(),
            world_spawn: false,
            spawn_order: 0,
        })
        .collect();

    // Step 4: tag the largest island as world spawn.
    let spawn_centroid = if let Some(spawn) = islands.iter_mut().max_by_key(|i| i.city_count) {
        spawn.world_spawn = true;
        eprintln!("Tagged island {} as world spawn ({} cities)", spawn.id, spawn.city_count);
        spawn.centroid
    } else {
        // No islands at all -- nothing more to do.
        return islands;
    };

    // Step 5: rank remaining islands by distance from the spawn centroid.
    //
    // We use squared distance to avoid the sqrt -- the relative order is
    // identical and it keeps the arithmetic in integer space.
    let (sx, sy) = (spawn_centroid.0 as i64, spawn_centroid.1 as i64);

    let mut non_spawn: Vec<&mut Island> = islands
        .iter_mut()
        .filter(|i| !i.world_spawn)
        .collect();

    non_spawn.sort_by_key(|i| {
        let dx = i.centroid.0 as i64 - sx;
        let dy = i.centroid.1 as i64 - sy;
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

/// Load a chunk into the cache if it isn't already there.
pub(crate) fn ensure_chunk(
    reader: &ChunkedWorldReader,
    cache: &mut HashMap<(u32, u32), ChunkData>,
    cx: u32,
    cy: u32,
) {
    if !cache.contains_key(&(cx, cy)) {
        if let Ok(chunk) = reader.load_chunk(cx, cy) {
            cache.insert((cx, cy), chunk);
        }
    }
}

/// Look up the region label for a world coordinate, loading the containing
/// chunk if necessary.
fn region_label_at(
    reader: &ChunkedWorldReader,
    cache: &mut HashMap<(u32, u32), ChunkData>,
    chunk_size: u32,
    x: u32,
    y: u32,
) -> u32 {
    let (cx, cy) = (x / chunk_size, y / chunk_size);
    ensure_chunk(reader, cache, cx, cy);
    cache
        .get(&(cx, cy))
        .map(|chunk| {
            let lx = (x - cx * chunk_size) as usize;
            let ly = (y - cy * chunk_size) as usize;
            chunk.region_labels[ly * chunk.width as usize + lx]
        })
        .unwrap_or(0)
}
