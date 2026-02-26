//! Elevation map generation.
//!
//! Uses Perlin noise with fractal Brownian motion to produce a 2-D heightmap
//! normalised to `[0.0, 1.0]`. All parameters are drawn from [`WorldConfig`].

use noise::{NoiseFn, Perlin};
use rand::{RngExt, SeedableRng};
use rand::rngs::StdRng;

use crate::config::WorldConfig;

/// Generate a square elevation grid from the given configuration.
///
/// Each cell is in `[0.0, 1.0]` after normalisation.
pub fn generate(config: &WorldConfig) -> Vec<Vec<f64>> {
    let size = config.map_len();
    let perlin = Perlin::new(config.seed);

    // Use a seeded RNG so the offsets are deterministic for a given seed.
    let mut rng = StdRng::seed_from_u64(config.seed as u64);
    let offset_x = rng.random::<u32>() % 10_000;
    let offset_y = rng.random::<u32>() % 10_000;

    let mut elevation = vec![vec![0.0; size]; size];

    for y in 0..size {
        for x in 0..size {
            let mut freq = 1.0 / config.scale;
            let mut amp = 1.0;
            let mut noise_sum = 0.0;
            let mut amp_sum = 0.0;

            for _ in 0..config.octaves {
                let nx = (x as f64 + offset_x as f64) * freq;
                let ny = (y as f64 + offset_y as f64) * freq;
                noise_sum += perlin.get([nx, ny]) * amp;
                amp_sum += amp;
                amp *= config.persistence;
                freq *= config.lacunarity;
            }
            elevation[y][x] = (noise_sum / amp_sum + 1.0) / 2.0;
        }
    }
    elevation
}
