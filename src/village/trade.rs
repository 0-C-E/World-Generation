//! Village trade profile computation.
//!
//! Each village scans a circular neighbourhood of radius [`VILLAGE_SCAN_RADIUS`]
//! and aggregates the biome resource modifiers across all tiles in that area,
//! considering only the four tradeable resources (Wood, Stone, Food, Metal).
//!
//! The resource with the highest aggregate becomes `offers`; the lowest becomes
//! `demands`. Ties are broken deterministically via a position-seeded hash so
//! placement is stable across runs with no external RNG state required.

use super::{TradeResource, VillageTrade};
use crate::biome::Biome;

/// Number of tradeable resources. Must equal the number of `TradeResource` variants.
const NUM_TRADE_RESOURCES: usize = 4; // Wood, Stone, Food, Metal

/// Scan radius in tiles around a village centre.
///
/// Smaller than the city scan radius (4 vs 6) — villages control a tighter
/// hinterland, so two adjacent villages can reasonably have different trades.
pub const VILLAGE_SCAN_RADIUS: i32 = 4;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Compute the trade profile for a village at `(vx, vy)`.
///
/// Returns `None` only if the scan finds zero tiles in bounds — which can
/// never happen for a valid village position on a real map.
///
/// # Tie-breaking
///
/// When several resources share the same aggregate value, the winner is chosen
/// via a cheap position+seed hash, using different salt constants for `offers`
/// and `demands` to avoid always picking the same index.
///
/// # Guarantee: `offers != demands`
///
/// The function always returns two distinct resources, even when all totals
/// are equal, by excluding the `offers` winner from the `demands` candidate
/// pool before tie-breaking.
pub fn compute_village_trade(
    vx: usize,
    vy: usize,
    biomes: &[Vec<u8>],
    seed: u32,
) -> Option<VillageTrade> {
    let map_h = biomes.len();
    let map_w = if map_h > 0 {
        biomes[0].len()
    } else {
        return None;
    };
    let r = VILLAGE_SCAN_RADIUS;

    // Aggregate only the four tradeable biome modifiers: wood, stone, food, metal.
    // Favor (index 4) is deliberately excluded.
    let mut totals = [0i32; NUM_TRADE_RESOURCES];

    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let tx = vx as i32 + dx;
            let ty = vy as i32 + dy;
            if tx < 0 || ty < 0 || tx >= map_w as i32 || ty >= map_h as i32 {
                continue;
            }
            let m = Biome::from_u8(biomes[ty as usize][tx as usize]).resource_modifiers();
            totals[0] += m.wood as i32;
            totals[1] += m.stone as i32;
            totals[2] += m.food as i32;
            totals[3] += m.metal as i32;
        }
    }

    let max_val = *totals.iter().max()?;
    let min_val = *totals.iter().min()?;

    let max_candidates: Vec<usize> = (0..NUM_TRADE_RESOURCES)
        .filter(|&i| totals[i] == max_val)
        .collect();
    let min_candidates: Vec<usize> = (0..NUM_TRADE_RESOURCES)
        .filter(|&i| totals[i] == min_val)
        .collect();

    let offers_idx = tie_break(&max_candidates, vx, vy, seed, 0x0F3D_5EED);

    // Exclude `offers` from the demands pool so they are always distinct.
    let filtered_min: Vec<usize> = min_candidates
        .iter()
        .copied()
        .filter(|&i| i != offers_idx)
        .collect();

    let demands_idx = if filtered_min.is_empty() {
        // All four resources are equal — pick any resource other than offers.
        let others: Vec<usize> = (0..NUM_TRADE_RESOURCES)
            .filter(|&i| i != offers_idx)
            .collect();
        tie_break(&others, vx, vy, seed, 0xDA4D_5EED)
    } else {
        tie_break(&filtered_min, vx, vy, seed, 0xDA4D_5EED)
    };

    Some(VillageTrade {
        offers: TradeResource::from_u8(offers_idx as u8),
        demands: TradeResource::from_u8(demands_idx as u8),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pick one element from `candidates` deterministically using a position-seeded
/// hash. Different `salt` values produce independent choices.
fn tie_break(candidates: &[usize], x: usize, y: usize, seed: u32, salt: u32) -> usize {
    if candidates.len() == 1 {
        return candidates[0];
    }
    let h = (x as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(y as u32)
        .wrapping_mul(0x85EB_CA6B)
        .wrapping_add(seed)
        .wrapping_mul(0xC2B2_AE35)
        .wrapping_add(salt);
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x45D9_F3B7);
    let h = h ^ (h >> 16);
    candidates[(h as usize) % candidates.len()]
}
