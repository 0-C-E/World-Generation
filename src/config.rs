//! World configuration.
//!
//! [`WorldConfig`] holds every tunable parameter for the generation pipeline.
//! Values are read from environment variables (with `.env` loaded via
//! `dotenvy` in dev). Unset variables fall back to sensible defaults, so a
//! bare `cargo run` works out of the box.
//!
//! To tweak parameters, set environment variables or edit `.env` and restart.
//! No recompilation needed.

use std::env;

/// All tunable parameters for world generation.
///
/// Use [`Default::default()`] for the standard 10,000 x 10,000 world.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    // -- Map geometry -------------------------------------------------------

    /// Side length of the square world in tiles.
    pub map_size: u16,
    /// Side length of one chunk in tiles (chunks are always square).
    pub chunk_size: u16,

    // -- Noise / elevation --------------------------------------------------

    /// Perlin noise seed.
    pub seed: u32,
    /// Base frequency of the noise (higher = more detail per tile).
    pub scale: f32,
    /// Number of fractal noise octaves.
    pub octaves: u8,
    /// Amplitude decay per octave.
    pub persistence: f32,
    /// Frequency multiplier per octave.
    pub lacunarity: f32,

    // -- Terrain classification ---------------------------------------------

    /// Elevation below this value is classified as water.
    pub water_threshold: f32,
    /// Maximum distance from the map center for the playable area.
    pub playable_radius: u16,
    /// Distance in tiles beyond `playable_radius` where
    /// [`FarLand`](crate::terrain::Terrain::FarLand) begins.
    /// Defaults to 2x `city_spacing`.
    pub farland_margin: u16,

    // -- City placement -----------------------------------------------------

    /// Minimum tile spacing between two city slots.
    pub city_spacing: u8,
    /// Islands with fewer candidate city slots than this are excluded.
    pub min_city_slots_per_island: u8,
    /// Minimum size (in tiles) of a neighbouring water body for a city to
    /// qualify as coastal.
    pub min_water_body_size: u16,
    /// Minimum number of land neighbours a tile must have to place a city.
    pub min_land_neighbors: u8,
    /// Minimum number of water neighbours a tile must have to place a city.
    pub min_water_neighbors: u8,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl WorldConfig {
    /// Build a configuration from environment variables.
    ///
    /// Every field falls back to a default when the variable is unset or
    /// empty. Call [`dotenvy::dotenv()`] before this to load `.env` files.
    pub fn from_env() -> Self {
        let map_size = env_u16("MAP_SIZE", 10_000);

        let chunk_size = match env::var("CHUNK_SIZE").ok().as_deref() {
            Some("auto") | Some("") | None => Self::optimal_chunk_size(map_size),
            Some(v) => v.parse::<u16>().unwrap_or_else(|_| {
                eprintln!("CHUNK_SIZE: invalid value \"{v}\", using auto");
                Self::optimal_chunk_size(map_size)
            }),
        };

        let seed = match env::var("SEED").ok().as_deref() {
            Some("") | None => rand::random::<u32>(),
            Some(v) => v.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("SEED: invalid value \"{v}\", using random");
                rand::random::<u32>()
            }),
        };

        let city_spacing = env_u8("CITY_SPACING", 5);
        let radius_frac = env_f32("PLAYABLE_RADIUS_FRAC", 0.975);
        let farland_margin = env_u16("FARLAND_MARGIN", city_spacing as u16 * 2);

        Self {
            map_size,
            chunk_size,
            seed,
            scale: env_f32("SCALE", 50.0),
            octaves: env_u8("OCTAVES", 6),
            persistence: env_f32("PERSISTENCE", 0.5),
            lacunarity: env_f32("LACUNARITY", 2.5),
            water_threshold: env_f32("WATER_THRESHOLD", 0.55),
            playable_radius: ((map_size as f32 / 2.0) * radius_frac) as u16,
            farland_margin,
            city_spacing,
            min_city_slots_per_island: env_u8("MIN_CITY_SLOTS_PER_ISLAND", 6),
            min_water_body_size: env_u16("MIN_WATER_BODY_SIZE", 500),
            min_land_neighbors: env_u8("MIN_LAND_NEIGHBORS", 2),
            min_water_neighbors: env_u8("MIN_WATER_NEIGHBORS", 2),
        }
    }
    /// Map size as `usize` -- avoids casts in hot loops.
    pub fn map_len(&self) -> usize {
        self.map_size as usize
    }

    /// Best chunk size for a given map size.
    ///
    /// Returns the largest power of two <= `map_size / 20`, clamped to
    /// [16, 512]. Keeps chunk count manageable while staying small enough
    /// for lazy loading.
    ///
    /// | `map_size` | chunk_size |
    /// |------------|------------|
    /// |     100    |     16     |
    /// |     500    |     16     |
    /// |   1 000    |     32     |
    /// |   5 000    |    128     |
    /// |  10 000    |    256     |
    /// |  20 000    |    512     |
    pub fn optimal_chunk_size(map_size: u16) -> u16 {
        let target = map_size as u32 / 20;
        if target < 16 {
            return 16;
        }
        // Largest power of two <= target
        let pow2 = 1u32 << (31 - target.leading_zeros());
        pow2.min(512) as u16
    }

    /// Max zoom level for this map size.
    ///
    /// `floor(log2(map_size)) - 5`, clamped to [1, 10].
    /// At max zoom each tile covers roughly 30-60 world tiles.
    pub fn max_zoom(&self) -> u32 {
        if self.map_size <= 1 {
            return 1;
        }
        let log2 = 15 - self.map_size.leading_zeros(); // floor(log2)
        (log2.saturating_sub(5)).clamp(1, 10)
    }

    /// Rendered tile side-length in pixels (always 256 for Leaflet).
    pub const fn tile_pixel_size() -> u32 {
        256
    }
}

// ---------------------------------------------------------------------------
// Env-var helpers
// ---------------------------------------------------------------------------

fn env_u16(key: &str, default: u16) -> u16 {
    match env::var(key).ok().as_deref() {
        Some("") | None => default,
        Some(v) => v.parse().unwrap_or_else(|_| {
            eprintln!("{key}: invalid value \"{v}\", using default {default}");
            default
        }),
    }
}

fn env_u8(key: &str, default: u8) -> u8 {
    match env::var(key).ok().as_deref() {
        Some("") | None => default,
        Some(v) => v.parse().unwrap_or_else(|_| {
            eprintln!("{key}: invalid value \"{v}\", using default {default}");
            default
        }),
    }
}

fn env_f32(key: &str, default: f32) -> f32 {
    match env::var(key).ok().as_deref() {
        Some("") | None => default,
        Some(v) => v.parse().unwrap_or_else(|_| {
            eprintln!("{key}: invalid value \"{v}\", using default {default}");
            default
        }),
    }
}
