//! Elevation map generation using fractal Brownian motion.
//!
//! # Overview
//!
//! Produces a 2D heightmap using **Perlin noise** with **fractal Brownian motion (fBm)**,
//! creating natural-looking terrain with both large landmasses and fine coastal detail.
//!
//! # How it works
//!
//! For each tile, we:
//! 1. Sum multiple "octaves" of Perlin noise at different frequencies and amplitudes
//! 2. Each octave is scaled by `persistence` (amplitude decay) and `lacunarity` (frequency growth)
//! 3. Normalize the result to `[0.0, 1.0]`
//!
//! This produces a natural fractal pattern where:
//! - Large-scale mountains and continents emerge from low-frequency octaves
//! - Fine coastal detail comes from high-frequency octaves
//! - The balance between scales is controlled by `persistence` and `lacunarity`
//!
//! # Parameters
//!
//! - `scale`: Base frequency of the lowest octave (higher = more detailed)
//! - `octaves`: Number of noise layers to sum (more = longer to compute, finer detail)
//! - `persistence`: Amplitude multiplier per octave (0.5 = half amplitude each octave)
//! - `lacunarity`: Frequency multiplier per octave (2.5 = 2.5× frequency each octave)
//!
//! # Example
//!
//! ```ignore
//! let config = WorldConfig::default();
//! let elevation = generate(&config);
//! // elevation[y][x] ∈ [0.0, 1.0] for all tiles
//! ```

use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

use crate::config::WorldConfig;

/// Generate a square elevation grid using fractal Brownian motion Perlin noise.
///
/// # Returns
///
/// A 2D vector indexed as `elevation[y][x]` with values in `[0.0, 1.0]`.
/// Water typically occupies the range `[0.0, water_threshold)` in the next step.
///
/// # Algorithm
///
/// For each tile `(x, y)`:
/// 1. Apply a deterministic offset based on the seed RNG
/// 2. Sum multiple octaves of Perlin noise:
///    - Start with frequency `1.0 / scale` and amplitude `1.0`
///    - Each octave: `noise_sum += perlin.get([nx, ny]) * amplitude`
///    - Update: `amplitude *= persistence`, `frequency *= lacunarity`
/// 3. Normalize: `(noise_sum / amplitude_sum + 1.0) / 2.0` to map from noise range to `[0.0, 1.0]`
pub fn generate(config: &WorldConfig) -> Vec<Vec<f64>> {
    let size = config.map_len();
    let perlin = Perlin::new(config.seed);

    // Use a seeded RNG so the offsets are deterministic for a given seed.
    let mut rng = StdRng::seed_from_u64(config.seed as u64);
    let offset_x = rng.random::<u32>() % 10_000;
    let offset_y = rng.random::<u32>() % 10_000;

    // Widen to f64 once -- the noise function requires f64 precision.
    let scale = config.scale as f64;
    let persistence = config.persistence as f64;
    let lacunarity = config.lacunarity as f64;

    let mut elevation = vec![vec![0.0; size]; size];

    elevation.par_iter_mut().enumerate().for_each(|(y, row)| {
        for x in 0..size {
            let mut freq = 1.0 / scale;
            let mut amp = 1.0;
            let mut noise_sum = 0.0;
            let mut amp_sum = 0.0;

            for _ in 0..config.octaves {
                let nx = (x as f64 + offset_x as f64) * freq;
                let ny = (y as f64 + offset_y as f64) * freq;
                noise_sum += perlin.get([nx, ny]) * amp;
                amp_sum += amp;
                amp *= persistence;
                freq *= lacunarity;
            }
            row[x] = (noise_sum / amp_sum + 1.0) / 2.0;
        }
    });
    elevation
}
