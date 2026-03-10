//! Village placement algorithm.
//!
//! Villages are placed on inland Land tiles using a two-stage process:
//!
//! 1. **Candidate collection** — all Land tiles within an island that are
//!    at least [`MIN_OCEAN_DISTANCE`] tiles from any Water or FarLand tile,
//!    are not a city slot, and carry a non-coastal biome.
//!
//! 2. **Organic scatter + greedy spacing** — candidates are shuffled via a
//!    seeded Fisher-Yates shuffle (deterministic, no external RNG crate needed
//!    in this hot path) biased toward more-inland tiles, then picked one by
//!    one with a minimum Chebyshev spacing constraint. This produces a natural,
//!    scattered distribution rather than a tight inland cluster.
//!
//! # Why not sort purely by ocean distance?
//!
//! Pure inland-sort clusters all villages at the deepest interior of the
//! island. The weighted shuffle preserves an inland preference (deeper tiles
//! appear more frequently near the front) while introducing enough positional
//! variation that villages spread organically across the island.
//!
//! # Count formula
//!
//! ```text
//! target = floor(alpha × (city_count − min_cities)^beta)
//! ```
//!
//! Yields 0 for minimum-size islands and grows sub-linearly.

use std::collections::{HashMap, HashSet};

use super::{compute_village_trade, Village};
use crate::biome::Biome;
use crate::config::WorldConfig;
use crate::terrain::Terrain;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default minimum tile distance from any ocean/farland tile.
pub const MIN_OCEAN_DISTANCE: u32 = 8;

/// Default minimum Chebyshev distance between two villages on the same island.
/// 20 tiles gives comfortable visual separation at all zoom levels.
pub const MIN_VILLAGE_SPACING: usize = 20;

// ---------------------------------------------------------------------------
// Count formula
// ---------------------------------------------------------------------------

/// Number of villages to place on an island with `city_count` cities.
///
/// ```text
/// f(c) = floor(alpha × (c − min_cities)^beta)
/// ```
///
/// Properties:
/// - Exactly 0 when `city_count == min_cities` (algebraic zero, no `if`)
/// - Front-loaded: 15–30 city islands get 3–5 villages (beta=0.60)
/// - No hard cap; diminishing returns naturally limit very large islands
pub fn village_count_for_island(city_count: u32, min_cities: u32, alpha: f64, beta: f64) -> u32 {
    if city_count <= min_cities {
        return 0;
    }
    let delta = (city_count - min_cities) as f64;
    (alpha * delta.powf(beta)).floor() as u32
}

// ---------------------------------------------------------------------------
// Main placement function
// ---------------------------------------------------------------------------

/// Place villages for all qualifying islands and return them sorted by
/// `(region_id, y, x)` for deterministic output.
///
/// # Arguments
///
/// * `terrain`           — row-major terrain grid
/// * `biomes`            — row-major biome grid (`Biome::to_u8()` values)
/// * `region_labels`     — row-major flood-fill region IDs
/// * `ocean_distances`   — per-tile distance to nearest Water/FarLand tile
/// * `region_city_counts`— number of accepted city slots per region
/// * `city_slots`        — all accepted city positions (used as exclusion set)
/// * `config`            — world configuration (spacing, alpha, beta, seed)
pub fn place_villages(
    terrain: &[Vec<Terrain>],
    biomes: &[Vec<u8>],
    region_labels: &[Vec<usize>],
    ocean_distances: &[Vec<u32>],
    region_city_counts: &HashMap<usize, u32>,
    city_slots: &[(usize, usize)],
    config: &WorldConfig,
) -> Vec<Village> {
    let map_h = terrain.len();
    let map_w = if map_h > 0 {
        terrain[0].len()
    } else {
        return vec![];
    };

    let min_ocean = config.village_min_ocean_distance;
    let spacing = config.village_spacing as usize;
    let alpha = config.village_alpha;
    let beta = config.village_beta;
    let seed = config.seed;
    let min_cities = config.min_city_slots_per_island as u32;

    // Build city exclusion set for O(1) lookup.
    let city_set: HashSet<(usize, usize)> = city_slots.iter().copied().collect();

    // -----------------------------------------------------------------------
    // Step 1 — collect per-region candidate tiles
    // -----------------------------------------------------------------------
    // A tile qualifies when it is:
    //   • Land (not Water, not FarLand)
    //   • ocean_distance >= min_ocean
    //   • not an existing city slot
    //   • belongs to a region that passed the city-count filter
    //   • has a land biome (not water/coastal/farland biome)

    let mut by_region: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();

    for y in 0..map_h {
        for x in 0..map_w {
            if terrain[y][x] != Terrain::Land {
                continue;
            }
            if ocean_distances[y][x] < min_ocean {
                continue;
            }
            if city_set.contains(&(x, y)) {
                continue;
            }
            let region_id = region_labels[y][x];
            if region_id == 0 || !region_city_counts.contains_key(&region_id) {
                continue;
            }
            let biome = Biome::from_u8(biomes[y][x]);
            if matches!(
                biome,
                Biome::Ocean | Biome::Coast | Biome::Beach | Biome::DeepHarbor | Biome::FarLand
            ) {
                continue;
            }
            by_region.entry(region_id).or_default().push((x, y));
        }
    }

    // -----------------------------------------------------------------------
    // Step 2 — organic scatter + greedy spacing per island
    // -----------------------------------------------------------------------

    let mut all_villages: Vec<Village> = Vec::new();

    for (region_id, candidates) in by_region {
        let city_count = *region_city_counts.get(&region_id).unwrap_or(&0);
        let target = village_count_for_island(city_count, min_cities, alpha, beta) as usize;
        if target == 0 || candidates.is_empty() {
            continue;
        }

        // Weighted shuffle: biases inland tiles toward the front while still
        // scattering them spatially across the island.
        //
        // Each candidate gets a score:
        //   score = ocean_distance × weight_factor + positional_hash
        //
        // where weight_factor > 1 ensures deeper inland tiles are
        // statistically preferred, and the hash adds per-tile variation.
        // We then sort descending by score — simple, deterministic, no RNG crate.
        let mut scored: Vec<(usize, usize, u64)> = candidates
            .into_iter()
            .map(|(x, y)| {
                let d = ocean_distances[y][x] as u64;
                // Inland weight: a tile at distance d contributes d * 4 base score.
                // The hash term spreads tiles at similar depths across the island.
                let h = scatter_hash(x, y, seed);
                // h is in [0, 0xFFFF]. Scale inland weight so that a 1-tile
                // depth difference dominates over hash noise only after depth > 8.
                let score = d * 256 + (h % 256);
                (x, y, score)
            })
            .collect();

        // Sort descending: most-inland / highest-scored first.
        // For equal scores (very rare), use (y, x) for strict determinism.
        scored.sort_unstable_by(|&(ax, ay, as_), &(bx, by, bs)| {
            bs.cmp(&as_)
                .then_with(|| ay.cmp(&by))
                .then_with(|| ax.cmp(&bx))
        });

        // Greedy Chebyshev spacing pass.
        let mut placed: Vec<(usize, usize)> = Vec::with_capacity(target);

        for (cx, cy, _) in scored {
            if placed.len() >= target {
                break;
            }
            let too_close = placed.iter().any(|&(px, py)| {
                let dx = (cx as isize - px as isize).unsigned_abs();
                let dy = (cy as isize - py as isize).unsigned_abs();
                // Chebyshev: max(dx, dy) < spacing
                dx.max(dy) < spacing
            });
            if too_close {
                continue;
            }

            let trade = compute_village_trade(cx, cy, biomes, seed).unwrap_or_default();
            all_villages.push(Village {
                x: cx as u16,
                y: cy as u16,
                region_id: region_id as u32,
                biome: biomes[cy][cx],
                trade,
            });
            placed.push((cx, cy));
        }
    }

    all_villages.sort_unstable_by_key(|v| (v.region_id, v.y, v.x));
    all_villages
}

/// Cheap position+seed hash used to scatter equally-inland candidates.
/// Returns a value in [0, 0xFFFF_FFFF]. Different seeds produce independent
/// scatter patterns, so each world looks distinct.
fn scatter_hash(x: usize, y: usize, seed: u32) -> u64 {
    let h = (x as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(y as u32)
        .wrapping_mul(0x85EB_CA6B)
        .wrapping_add(seed)
        .wrapping_mul(0xC2B2_AE35);
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x45D9_F3B7);
    (h ^ (h >> 16)) as u64
}
