//! Gold vein detection via Perlin noise contours.
//!
//! Gold veins follow the **zero-contour** of a dedicated noise field:
//! wherever `|noise(x, y)| < GOLD_VEIN_THRESHOLD` the tile is considered
//! a vein. Because Perlin contour lines are smooth and connected this
//! produces thin, river-like veins that meander organically across the map.
//!
//! [`NoiseLayer`] is also reused by [`generation`](super::generation) for the
//! biome classification layers, which is why it lives here rather than being
//! inlined into [`GoldVeinSampler`].

use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::biome::Biome;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// High frequency → dense, thin veins.
const GOLD_VEIN_FREQ: f64 = 0.05;

/// Half-width of the noise band counted as a vein (traces the zero-contour).
/// Smaller = thinner veins.
const GOLD_VEIN_THRESHOLD: f64 = 0.02;

const LAYER_OCTAVES: u32 = 5;
const LAYER_PERSISTENCE: f64 = 0.5;
const LAYER_LACUNARITY: f64 = 2.0;

// ---------------------------------------------------------------------------
// NoiseLayer
// ---------------------------------------------------------------------------

/// A single Perlin noise layer with its own seed, random offsets, and base
/// frequency.
///
/// `pub(crate)` so [`generation`](super::generation) can borrow it for biome
/// classification without duplicating the octave-blending logic.
pub(crate) struct NoiseLayer {
    perlin: Perlin,
    offset_x: f64,
    offset_y: f64,
    base_freq: f64,
}

impl NoiseLayer {
    pub(crate) fn new(seed: u32, base_freq: f64, rng: &mut StdRng) -> Self {
        Self {
            perlin: Perlin::new(seed),
            offset_x: (rng.random::<u32>() % 10_000) as f64,
            offset_y: (rng.random::<u32>() % 10_000) as f64,
            base_freq,
        }
    }

    /// Sample at world coordinate `(x, y)` using fractal octave blending.
    ///
    /// Returns a value in approximately `[-1, 1]`.
    pub(crate) fn sample(&self, x: usize, y: usize) -> f64 {
        let mut value = 0.0f64;
        let mut amp = 1.0f64;
        let mut amp_sum = 0.0f64;
        let mut freq = self.base_freq;

        for _ in 0..LAYER_OCTAVES {
            let nx = (x as f64 + self.offset_x) * freq;
            let ny = (y as f64 + self.offset_y) * freq;
            value += self.perlin.get([nx, ny]) * amp;
            amp_sum += amp;
            amp *= LAYER_PERSISTENCE;
            freq *= LAYER_LACUNARITY;
        }

        value / amp_sum
    }
}

// ---------------------------------------------------------------------------
// GoldVeinSampler
// ---------------------------------------------------------------------------

/// Samples a dedicated Perlin noise layer to detect gold vein tiles.
///
/// Create once per world from the world seed, then reuse for every query.
pub struct GoldVeinSampler {
    layer: NoiseLayer,
}

impl GoldVeinSampler {
    pub fn new(seed: u32) -> Self {
        let mut rng = StdRng::seed_from_u64(seed as u64 ^ 0x601D_AE17);
        let layer = NoiseLayer::new(
            seed.wrapping_mul(13).wrapping_add(7),
            GOLD_VEIN_FREQ,
            &mut rng,
        );
        Self { layer }
    }

    /// Returns `true` if `(x, y)` lies on a gold vein in a gold-eligible biome.
    pub fn is_gold(&self, x: usize, y: usize, biome: Biome) -> bool {
        biome.has_gold_veins() && self.layer.sample(x, y).abs() < GOLD_VEIN_THRESHOLD
    }
}
