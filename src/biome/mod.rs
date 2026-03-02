//! Biome classification and per-biome data.
//!
//! # Adding a new biome
//!
//! 1. Create `src/biome/defs/my_biome.rs` with `pub const DATA: BiomeData`.
//! 2. Add `pub mod my_biome;` to `src/biome/defs/mod.rs`.
//! 3. Add a variant to [`Biome`] — use the **next available `u8`** (currently
//!    16+). Never reorder or reuse discriminants; they are persisted in the
//!    world file binary format.
//! 4. Add one arm to [`Biome::from_u8`] and one to [`Biome::data`].
//! 5. Add a classification rule in [`generation`].
//!
//! That's it. No other files need to change.

pub mod city_resources;
pub mod defs;
pub mod generation;
pub mod gold;

// Re-export the most commonly used types so callers can write
// `biome::generate_biomes` / `biome::CityResources` without extra path depth.
pub use city_resources::{compute_city_resources, CityResources};
pub use generation::generate_biomes;
pub use gold::GoldVeinSampler;

use crate::terrain::Terrain;

// ---------------------------------------------------------------------------
// ResourceModifiers
// ---------------------------------------------------------------------------

/// Per-biome passive resource production modifiers (percentage points).
///
/// Positive = bonus, negative = malus. Gold is deliberately absent — it is
/// not a passive resource. See [`Biome::has_gold_veins`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceModifiers {
    pub wood:  i8,
    pub stone: i8,
    pub food:  i8,
    pub metal: i8,
    pub favor: i8,
}

impl ResourceModifiers {
    pub const fn new(wood: i8, stone: i8, food: i8, metal: i8, favor: i8) -> Self {
        Self { wood, stone, food, metal, favor }
    }
}

// ---------------------------------------------------------------------------
// BiomeData
// ---------------------------------------------------------------------------

/// All properties of a single biome, defined as a `const` in its own file.
///
/// Color is encoded as function pointers so it can live in `const` context:
/// - `land_color`:  receives `t` — elevation rescaled to `[0, 1]` over the
///   land range via `((raw_elevation - 0.5) / 0.5).clamp(0, 1)`.
/// - `water_color`: receives `(raw_elevation, water_threshold)`.
/// - `None` means the biome never appears in that terrain category.
pub struct BiomeData {
    pub name:          &'static str,
    pub modifiers:     ResourceModifiers,
    pub has_gold_veins: bool,
    pub land_color:    Option<fn(f32) -> [u8; 3]>,
    pub water_color:   Option<fn(f32, f32) -> [u8; 3]>,
}

// ---------------------------------------------------------------------------
// Biome enum
// ---------------------------------------------------------------------------

/// Tile biome classification.
///
/// Discriminants are part of the saved world format — **append-only**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Biome {
    Ocean       = 0,
    Coast       = 1,
    Beach       = 2,
    Plains      = 3,
    Forest      = 4,
    Swamp       = 5,
    Hills       = 6,
    Mountains   = 7,
    SnowyPeaks  = 8,
    Desert      = 9,
    Tundra      = 10,
    Valley      = 11,
    Highlands   = 12,
    SacredGrove = 13,
    DeepHarbor  = 14,
    FarLand     = 15,
}

impl Biome {
    // -----------------------------------------------------------------------
    // Single dispatch point
    // -----------------------------------------------------------------------

    /// Return the [`BiomeData`] for this biome.
    ///
    /// Every other method on `Biome` delegates here — adding a new biome
    /// only requires adding one arm here (and one in `from_u8`).
    pub fn data(self) -> &'static BiomeData {
        match self {
            Biome::Ocean       => &defs::ocean::DATA,
            Biome::Coast       => &defs::coast::DATA,
            Biome::Beach       => &defs::beach::DATA,
            Biome::Plains      => &defs::plains::DATA,
            Biome::Forest      => &defs::forest::DATA,
            Biome::Swamp       => &defs::swamp::DATA,
            Biome::Hills       => &defs::hills::DATA,
            Biome::Mountains   => &defs::mountains::DATA,
            Biome::SnowyPeaks  => &defs::snowy_peaks::DATA,
            Biome::Desert      => &defs::desert::DATA,
            Biome::Tundra      => &defs::tundra::DATA,
            Biome::Valley      => &defs::valley::DATA,
            Biome::Highlands   => &defs::highlands::DATA,
            Biome::SacredGrove => &defs::sacred_grove::DATA,
            Biome::DeepHarbor  => &defs::deep_harbor::DATA,
            Biome::FarLand     => &defs::far_land::DATA,
        }
    }

    // -----------------------------------------------------------------------
    // Convenience accessors
    // -----------------------------------------------------------------------

    pub fn name(self) -> &'static str {
        self.data().name
    }

    pub fn resource_modifiers(self) -> &'static ResourceModifiers {
        &self.data().modifiers
    }

    pub fn has_gold_veins(self) -> bool {
        self.data().has_gold_veins
    }

    // -----------------------------------------------------------------------
    // Color — replaces the old `color::get_color()` free function
    // -----------------------------------------------------------------------

    /// Map terrain + elevation to an RGB colour.
    ///
    /// This is the sole replacement for the old `color.rs` module. Call sites
    /// in `tile.rs` change from:
    /// ```ignore
    /// color::get_color(terrain, elevation, water_threshold, biome)
    /// ```
    /// to:
    /// ```ignore
    /// biome.get_color(terrain, elevation, water_threshold)
    /// ```
    pub fn get_color(self, terrain: Terrain, elevation: f32, water_threshold: f32) -> [u8; 3] {
        match terrain {
            Terrain::Water => {
                if let Some(f) = self.data().water_color {
                    f(elevation, water_threshold)
                } else {
                    // Generic ocean fallback (should not normally be reached).
                    let t = ((elevation - 0.10) / (water_threshold - 0.10))
                        .clamp(0.0, 1.0)
                        .powf(0.7);
                    [
                        ((1.0 - t) * 10.0 + t * 80.0) as u8,
                        ((1.0 - t) * 35.0 + t * 170.0) as u8,
                        ((1.0 - t) * 80.0 + t * 200.0) as u8,
                    ]
                }
            }
            Terrain::Land => {
                let t = ((elevation - 0.5) / 0.5).clamp(0.0, 1.0);
                if let Some(f) = self.data().land_color {
                    f(t)
                } else {
                    // Generic green fallback (should not normally be reached).
                    [
                        (90.0 + t * 60.0) as u8,
                        (140.0 + t * 40.0) as u8,
                        (50.0 + t * 40.0) as u8,
                    ]
                }
            }
            Terrain::FarLand => {
                // FarLand colour uses raw elevation, not the land-rescaled `t`.
                // It is uniform across all biomes, so it lives here rather than
                // in a per-biome file.
                let t = (elevation - 0.8) / 0.2;
                [
                    (200.0 + t * 55.0) as u8,
                    (180.0 + t * 75.0) as u8,
                    (160.0 + t * 95.0) as u8,
                ]
            }
        }
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Deserialize from the `u8` stored in chunk data.
    /// Unknown values fall back to [`Biome::Ocean`].
    pub fn from_u8(v: u8) -> Self {
        match v {
            0  => Biome::Ocean,
            1  => Biome::Coast,
            2  => Biome::Beach,
            3  => Biome::Plains,
            4  => Biome::Forest,
            5  => Biome::Swamp,
            6  => Biome::Hills,
            7  => Biome::Mountains,
            8  => Biome::SnowyPeaks,
            9  => Biome::Desert,
            10 => Biome::Tundra,
            11 => Biome::Valley,
            12 => Biome::Highlands,
            13 => Biome::SacredGrove,
            14 => Biome::DeepHarbor,
            15 => Biome::FarLand,
            _  => Biome::Ocean,
        }
    }
}
