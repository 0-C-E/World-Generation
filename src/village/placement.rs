//! Village placement algorithm.
//!
//! Villages are placed on inland Land tiles using a two-stage process:
//!
//! 1. **Candidate collection** — all Land tiles within an island that are
//!    at least [`MIN_OCEAN_DISTANCE`] tiles from any Water or FarLand tile,
//!    are not a city slot, and carry a land biome.
//!
//! 2. **Greedy selection** — candidates are sorted by ocean distance
//!    (most inland first), then picked one by one, skipping any tile that is
//!    within [`MIN_VILLAGE_SPACING`] of an already-placed village on the same
//!    island.  Selection stops when the island's target count is reached.
//!
//! The target count per island is given by the formula
//!
//! ```text
//! target = floor(alpha × (city_count − min_cities)^beta)
//! ```
//!
//! which yields 0 for minimum-size islands and grows sub-linearly, front-
//! loading villages onto medium-sized islands (15–30 cities).

use std::collections::{HashMap, HashSet};

use crate::biome::Biome;
use crate::config::WorldConfig;
use crate::terrain::Terrain;
use super::{Village, compute_village_trade};

// ---------------------------------------------------------------------------
// Placement constants (overridden via WorldConfig)
// ---------------------------------------------------------------------------

/// Default minimum tile distance from any ocean/farland tile.
/// Ensures villages feel genuinely inland.
pub const MIN_OCEAN_DISTANCE: u32 = 8;

/// Default minimum tile gap between two villages on the same island.
/// Chebyshev distance (max of |dx|, |dy|) — simple and cache-friendly.
pub const MIN_VILLAGE_SPACING: usize = 7;

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
    terrain:            &[Vec<Terrain>],
    biomes:             &[Vec<u8>],
    region_labels:      &[Vec<usize>],
    ocean_distances:    &[Vec<u32>],
    region_city_counts: &HashMap<usize, u32>,
    city_slots:         &[(usize, usize)],
    config:             &WorldConfig,
) -> Vec<Village> {
    let map_h = terrain.len();
    let map_w = if map_h > 0 { terrain[0].len() } else { return vec![] };

    let min_ocean = config.village_min_ocean_distance;
    let spacing   = config.village_spacing as usize;
    let alpha     = config.village_alpha;
    let beta      = config.village_beta;
    let seed      = config.seed;
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
            // Skip biomes that have no business hosting a village.
            let biome = Biome::from_u8(biomes[y][x]);
            if matches!(
                biome,
                Biome::Ocean | Biome::Coast | Biome::Beach
                | Biome::DeepHarbor | Biome::FarLand
            ) {
                continue;
            }

            by_region.entry(region_id).or_default().push((x, y));
        }
    }

    // -----------------------------------------------------------------------
    // Step 2 — greedy selection per island
    // -----------------------------------------------------------------------

    let mut all_villages: Vec<Village> = Vec::new();

    for (region_id, mut candidates) in by_region {
        let city_count = *region_city_counts.get(&region_id).unwrap_or(&0);
        let target = village_count_for_island(city_count, min_cities, alpha, beta) as usize;

        if target == 0 || candidates.is_empty() {
            continue;
        }

        // Sort most-inland first — this is our primary quality ordering.
        // Among equal ocean distances, sort by (y, x) for stability.
        candidates.sort_unstable_by(|&(ax, ay), &(bx, by)| {
            ocean_distances[by][bx]
                .cmp(&ocean_distances[ay][ax])
                .then_with(|| ay.cmp(&by))
                .then_with(|| ax.cmp(&bx))
        });

        let mut placed: Vec<(usize, usize)> = Vec::with_capacity(target);

        for (cx, cy) in candidates {
            if placed.len() >= target {
                break;
            }
            // Chebyshev spacing check against all already-placed villages on
            // this island. O(placed) — typically tiny.
            let too_close = placed.iter().any(|&(px, py)| {
                let dx = (cx as isize - px as isize).unsigned_abs();
                let dy = (cy as isize - py as isize).unsigned_abs();
                dx < spacing && dy < spacing
            });
            if too_close {
                continue;
            }

            let trade = compute_village_trade(cx, cy, biomes, seed)
                .unwrap_or_default();

            let base_rate = compute_base_rate(
                trade.offers,
                ocean_distances[cy][cx],
            );

            all_villages.push(Village {
                x:         cx as u16,
                y:         cy as u16,
                region_id: region_id as u32,
                base_rate,
                biome:     biomes[cy][cx],
                trade,
            });

            placed.push((cx, cy));
        }
    }

    // Stable sort for deterministic binary output.
    all_villages.sort_unstable_by_key(|v| (v.region_id, v.y, v.x));
    all_villages
}

// ---------------------------------------------------------------------------
// Base rate
// ---------------------------------------------------------------------------

/// Base production rate for a village in units/hour at island level 1 /
/// player level 1.
///
/// Favor villages run at a lower nominal rate because Favor is a rarer and
/// more valuable resource. Mixed villages are slightly below the standard
/// rate. All others start at 100 with an inland bonus of up to +20.
///
/// Range: approximately 50–220 units/hour.
fn compute_base_rate(offers: super::TradeResource, ocean_dist: u32) -> u16 {
    use super::TradeResource;

    let inland_bonus = (ocean_dist.min(30) as f64 / 30.0 * 20.0) as i32;

    let base: i32 = match offers {
        TradeResource::Favor => 55,
        _                    => 100,
    };

    (base + inland_bonus).clamp(50, 220) as u16
}
