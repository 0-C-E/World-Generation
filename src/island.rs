//! Island discovery and representation.
//!
//! An **island** is a connected region of [`Land`](crate::terrain::Terrain::Land)
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
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Scan city slots and chunk data to build the island registry.
///
/// Loads **all** chunks to compute accurate bounding boxes. The chunks remain
/// in `chunk_cache` afterward so later tile/outline requests are free.
pub fn discover_islands(
    reader: &ChunkedWorldReader,
    chunk_cache: &mut HashMap<(u32, u32), ChunkData>,
) -> Vec<Island> {
    let h = &reader.header;
    let cs = h.config.chunk_size;

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

    // Step 3: assemble Island structs.
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
        })
        .collect();
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
