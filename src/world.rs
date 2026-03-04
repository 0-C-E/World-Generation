//! High-level world access.
//!
//! [`World`] wraps a [`ChunkedWorldReader`](crate::save::ChunkedWorldReader)
//! with a chunk cache and lazy island discovery.

use std::collections::HashMap;
use std::io;

use crate::biome::CityResources;
use crate::config::WorldConfig;
use crate::island::{self, Island};
use crate::save::{ChunkData, ChunkedWorldReader};
use crate::village::Village;

pub struct World {
    reader: ChunkedWorldReader,
    chunk_cache: HashMap<(u32, u32), ChunkData>,
    islands: Option<Vec<Island>>,
}

impl World {
    /// Open a world file and read its header.
    pub fn open(path: &str) -> io::Result<Self> {
        let reader = ChunkedWorldReader::open(path)?;
        Ok(Self {
            reader,
            chunk_cache: HashMap::new(),
            islands: None,
        })
    }

    // -- Header accessors ---------------------------------------------------

    pub fn config(&self) -> &WorldConfig {
        &self.reader.header.config
    }
    pub fn width(&self) -> u32 {
        self.reader.header.width
    }
    pub fn height(&self) -> u32 {
        self.reader.header.height
    }
    pub fn chunks_x(&self) -> u32 {
        self.reader.header.chunks_x
    }
    pub fn chunks_y(&self) -> u32 {
        self.reader.header.chunks_y
    }

    /// All city slot positions from the file header.
    pub fn city_slots(&self) -> &[(u32, u32)] {
        &self.reader.header.city_slots
    }

    /// Per-city aggregated resource profiles, parallel to [`city_slots`](Self::city_slots).
    pub fn city_resources(&self) -> &[CityResources] {
        &self.reader.header.city_resources
    }
    /// All villages stored in the world file header.
    /// Returns an empty slice for version-1 files.
    pub fn villages(&self) -> &[Village] {
        &self.reader.header.villages
    }

    // -- Chunk management ---------------------------------------------------

    /// Load a chunk into the cache if it isn't already there.
    pub fn ensure_chunk(&mut self, cx: u32, cy: u32) -> io::Result<()> {
        if !self.chunk_cache.contains_key(&(cx, cy)) {
            let chunk = self.reader.load_chunk(cx, cy)?;
            self.chunk_cache.insert((cx, cy), chunk);
        }
        Ok(())
    }

    /// Get a reference to a cached chunk.
    ///
    /// Returns `None` if the chunk has not been loaded yet - call
    /// [`ensure_chunk`](Self::ensure_chunk) first.
    pub fn chunk(&self, cx: u32, cy: u32) -> Option<&ChunkData> {
        self.chunk_cache.get(&(cx, cy))
    }

    // -- Island discovery ---------------------------------------------------

    /// Discover all islands (loads every chunk into the cache).
    ///
    /// This is a no-op if islands have already been computed.
    pub fn ensure_islands_computed(&mut self) {
        if self.islands.is_none() {
            self.islands = Some(island::discover_islands(
                &self.reader,
                &mut self.chunk_cache,
            ));
        }
    }

    /// Return the discovered islands.
    ///
    /// Returns an empty slice if [`ensure_islands_computed`](Self::ensure_islands_computed)
    /// has not been called yet.
    pub fn islands(&self) -> &[Island] {
        self.islands.as_deref().unwrap_or(&[])
    }

    // -- Coordinate helpers -------------------------------------------------

    /// Look up the region label at `(x, y)` from **cached** chunks only.
    ///
    /// Returns `0` if the containing chunk is not in the cache.
    pub fn region_label_at_cached(&self, x: u32, y: u32) -> u32 {
        let cs = self.reader.header.config.chunk_size as u32;
        let (cx, cy) = (x / cs, y / cs);
        self.chunk(cx, cy)
            .map(|chunk| {
                let lx = (x - cx * cs) as usize;
                let ly = (y - cy * cs) as usize;
                chunk.region_labels[ly * chunk.width as usize + lx]
            })
            .unwrap_or(0)
    }

    /// Look up the region label at `(x, y)`, loading the chunk if needed.
    pub fn region_label_at(&mut self, x: u32, y: u32) -> u32 {
        let cs = self.reader.header.config.chunk_size as u32;
        let (cx, cy) = (x / cs, y / cs);
        let _ = self.ensure_chunk(cx, cy);
        self.region_label_at_cached(x, y)
    }
}
