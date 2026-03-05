//! Biome map generation.
//!
//! Drives five independent noise layers on top of the elevation grid and
//! classifies every tile into a [`Biome`] using threshold rules.
//!
//! # Classification priority (land tiles, most specific first)
//!
//! | # | Biome | Condition |
//! |---|-------|-----------|
//! | 1 | Beach | Barely above water threshold |
//! | 2 | Sacred Grove | High favor-harmony noise |
//! | 3 | Snowy Peaks | Very high elevation + cold |
//! | 4 | Mountains | High elevation |
//! | 5 | Desert | Hot + eroded |
//! | 6 | Tundra | Cold |
//! | 7 | Highlands | Peaks noise + moderately elevated |
//! | 8 | Valley | Valleys noise |
//! | 9 | Swamp | Very wet + low elevation |
//! | 10 | Forest | Low erosion |
//! | 11 | Hills | Moderate elevation |
//! | 12 | Plains | Default |

use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;

use crate::biome::{gold::NoiseLayer, Biome};
use crate::config::WorldConfig;
use crate::terrain::Terrain;

// ---------------------------------------------------------------------------
// Noise layer frequencies (world-space cycles per tile)
// ---------------------------------------------------------------------------

const CONTINENTALNESS_FREQ: f64 = 0.005;
const EROSION_FREQ: f64 = 0.015;
const TEMPERATURE_FREQ: f64 = 0.008;
const PEAKS_VALLEYS_FREQ: f64 = 0.030;
const FAVOR_FREQ: f64 = 0.003;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Generate biome classifications for every tile in the world.
///
/// Returns a row-major `Vec<Vec<u8>>` of [`Biome::to_u8`] values, parallel
/// to `terrain` and `elevation`.
pub fn generate_biomes(
    config: &WorldConfig,
    terrain: &[Vec<Terrain>],
    elevation: &[Vec<f64>],
) -> Vec<Vec<u8>> {
    let size = config.map_len();
    let base = config.seed;
    let mut rng = StdRng::seed_from_u64(base as u64 ^ 0xB10_E5EED);

    // Each layer gets a unique seed derived from the world seed so they are
    // independent but fully deterministic.
    let continentalness = NoiseLayer::new(
        base.wrapping_mul(7).wrapping_add(1),
        CONTINENTALNESS_FREQ,
        &mut rng,
    );
    let erosion = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(2), EROSION_FREQ, &mut rng);
    let temperature = NoiseLayer::new(
        base.wrapping_mul(7).wrapping_add(3),
        TEMPERATURE_FREQ,
        &mut rng,
    );
    let peaks_valleys = NoiseLayer::new(
        base.wrapping_mul(7).wrapping_add(4),
        PEAKS_VALLEYS_FREQ,
        &mut rng,
    );
    let favor = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(5), FAVOR_FREQ, &mut rng);

    let wt = config.water_threshold as f64;
    let mut biomes = vec![vec![0u8; size]; size];

    biomes.par_iter_mut().enumerate().for_each(|(y, row)| {
        for x in 0..size {
            let biome = match terrain[y][x] {
                Terrain::Water => classify_water(elevation[y][x], wt, continentalness.sample(x, y)),
                Terrain::Land => classify_land(
                    elevation[y][x],
                    wt,
                    erosion.sample(x, y),
                    temperature.sample(x, y),
                    peaks_valleys.sample(x, y),
                    favor.sample(x, y),
                ),
                Terrain::FarLand => Biome::FarLand,
            };
            row[x] = biome.to_u8();
        }
    });

    biomes
}

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

fn classify_water(elev: f64, water_threshold: f64, continentalness: f64) -> Biome {
    // Very low continentalness → deep ocean basins.
    if continentalness < -0.35 {
        return Biome::DeepHarbor;
    }
    // Close to the land/water boundary → shallow coastal water.
    if elev > water_threshold - 0.06 {
        return Biome::Coast;
    }
    Biome::Ocean
}

fn classify_land(
    elev: f64,
    water_threshold: f64,
    erosion: f64,
    temperature: f64,
    peaks_valleys: f64,
    favor: f64,
) -> Biome {
    let above_water = elev - water_threshold;

    if above_water < 0.02 {
        return Biome::Beach;
    }
    if favor > 0.55 {
        return Biome::SacredGrove;
    }
    if above_water > 0.28 && temperature < -0.15 {
        return Biome::SnowyPeaks;
    }
    if above_water > 0.22 {
        return Biome::Mountains;
    }
    if temperature > 0.35 && erosion > 0.15 {
        return Biome::Desert;
    }
    if temperature < -0.30 {
        return Biome::Tundra;
    }
    if peaks_valleys > 0.45 && above_water > 0.08 {
        return Biome::Highlands;
    }
    if peaks_valleys < -0.40 {
        return Biome::Valley;
    }
    if erosion < -0.30 && above_water < 0.08 {
        return Biome::Swamp;
    }
    if erosion < -0.05 {
        return Biome::Forest;
    }
    if above_water > 0.10 {
        return Biome::Hills;
    }

    Biome::Plains
}
