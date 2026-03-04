//! Village system — inland resource nodes on islands.
//!
//! Each village is placed on a Land tile that is sufficiently far from the
//! ocean, specialises in the resource its biome neighbourhood produces most,
//! and demands the resource that neighbourhood produces least.
//!
//! # Trade resources
//!
//! Only the four active-production resources (Wood, Stone, Food, Metal) are
//! tradeable. Favor and Gold are excluded: Favor is a passive divine resource
//! accumulated via temples, not traded; Gold is always actively farmed.
//!
//! # Adding a new trade resource
//!
//! 1. Add a variant to [`TradeResource`] — use the next available `u8`.
//!    Never reorder or reuse discriminants; they are persisted in the world
//!    file binary format.
//! 2. Add an arm to [`TradeResource::from_u8`] and [`TradeResource::name`].
//! 3. Bump the constant `NUM_TRADE_RESOURCES` in `trade.rs`.
//! 4. The rest (placement, trade) adapts automatically.

pub mod placement;
pub mod trade;

pub use placement::place_villages;
pub use trade::{compute_village_trade, VILLAGE_SCAN_RADIUS};

// ---------------------------------------------------------------------------
// TradeResource
// ---------------------------------------------------------------------------

/// The four resources a village can offer or demand in trade.
///
/// Favor and Gold are intentionally excluded — Favor is a passive divine
/// resource, Gold must be actively farmed; neither is traded between villages.
///
/// Discriminants are part of the saved world format — **append-only**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TradeResource {
    #[default]
    Wood = 0,
    Stone = 1,
    Food = 2,
    Metal = 3,
}

impl TradeResource {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wood => "Wood",
            Self::Stone => "Stone",
            Self::Food => "Food",
            Self::Metal => "Metal",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Wood => "🌲",
            Self::Stone => "⛰️",
            Self::Food => "🌾",
            Self::Metal => "⚙️",
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Unknown discriminants fall back to [`Wood`](TradeResource::Wood).
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Wood,
            1 => Self::Stone,
            2 => Self::Food,
            3 => Self::Metal,
            _ => Self::Wood,
        }
    }
}

// ---------------------------------------------------------------------------
// VillageTrade
// ---------------------------------------------------------------------------

/// What a village exports and what it wants in return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VillageTrade {
    /// Resource produced most in the surrounding biomes → exported.
    pub offers: TradeResource,
    /// Resource produced least in the surrounding biomes → imported.
    pub demands: TradeResource,
}

// ---------------------------------------------------------------------------
// Village
// ---------------------------------------------------------------------------

/// A fixed inland resource node, generated once per world.
///
/// Runtime state (island level, per-player trade progress) lives in the game
/// server database — not in the world file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Village {
    /// World x coordinate.
    pub x: u16,
    /// World y coordinate.
    pub y: u16,
    /// Region label of the island this village belongs to.
    pub region_id: u32,
    /// Dominant biome at the village's position (for display).
    pub biome: u8,
    /// Trade profile derived from the circular biome scan.
    pub trade: VillageTrade,
}
