//! World configuration.
//!
//! [`WorldConfig`] is the single source of truth for every tunable parameter
//! in the generation pipeline. Nothing is hardcoded - every module receives
//! the values it needs from this struct, making it straightforward to spin up
//! worlds with different characteristics (map size, density, terrain balance,
//! etc.).

/// Complete configuration for world generation and runtime behaviour.
///
/// Use [`Default::default()`] for the standard 10 000 x 10 000 world.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    // -- Map geometry -------------------------------------------------------

    /// Side length of the square world in tiles.
    pub map_size: u32,
    /// Side length of one chunk in tiles (chunks are always square).
    pub chunk_size: u32,

    // -- Noise / elevation --------------------------------------------------

    /// Perlin noise seed.
    pub seed: u32,
    /// Base frequency of the noise (higher -> more detail per tile).
    pub scale: f64,
    /// Number of fractal noise octaves.
    pub octaves: u32,
    /// Amplitude decay per octave.
    pub persistence: f64,
    /// Frequency multiplier per octave.
    pub lacunarity: f64,

    // -- Terrain classification ---------------------------------------------

    /// Elevation below this value is classified as water.
    pub water_threshold: f64,
    /// Maximum distance from the map center for the playable area.
    /// Tiles beyond this radius become [`FarLand`](crate::terrain::Terrain::FarLand).
    pub playable_radius: f64,

    // -- City placement -----------------------------------------------------

    /// Minimum tile spacing between two city slots.
    pub city_spacing: u32,
    /// Islands with fewer candidate city slots than this are excluded.
    pub min_city_slots_per_island: u32,
    /// Minimum size (in tiles) of a neighbouring water body for a city to
    /// qualify as coastal.
    pub min_water_body_size: u32,
    /// Minimum number of land neighbours a tile must have to place a city.
    pub min_land_neighbors: u32,
    /// Minimum number of water neighbours a tile must have to place a city.
    pub min_water_neighbors: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        let map_size = 10_000u32;
        Self {
            map_size,
            chunk_size: 256,
            seed: rand::random::<u32>(),
            scale: 50.0,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.5,
            water_threshold: 0.55,
            playable_radius: (map_size as f64 / 2.0) * 0.8,
            city_spacing: 5,
            min_city_slots_per_island: 6,
            min_water_body_size: 500,
            min_land_neighbors: 2,
            min_water_neighbors: 2,
        }
    }
}

impl WorldConfig {
    /// Map size as `usize` - avoids casts in hot loops.
    pub fn map_len(&self) -> usize {
        self.map_size as usize
    }
}
