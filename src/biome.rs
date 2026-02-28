//! Biome generation via layered Perlin noise.
//!
//! Uses 6 independent noise layers (continentalness, elevation, erosion,
//! temperature, peaks/valleys, favor harmony) with octave blending
//! (`sum(noise * persistence^octave)`) to classify every tile into a
//! strategically meaningful biome.
//!
//! # Noise layers
//!
//! | Layer | Frequency | Purpose |
//! |-------|-----------|---------|
//! | Continentalness | 0.005 | Island shape / landmass size |
//! | Elevation | (existing) | Height from plains to mountains |
//! | Erosion / Wetness | 0.015 | Fertility vs ruggedness |
//! | Temperature | 0.008 | Climate gradient (cold peaks to hot valleys) |
//! | Peaks / Valleys | 0.03 | Rare terrain features |
//! | Favor Harmony | 0.003 | Divine attunement zones |

use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::config::WorldConfig;
use crate::terrain::Terrain;

// ---------------------------------------------------------------------------
// Layer parameters
// ---------------------------------------------------------------------------

/// Frequency controls for each noise layer (world-space cycles per tile).
const CONTINENTALNESS_FREQ: f64 = 0.005;
const EROSION_FREQ: f64 = 0.015;
const TEMPERATURE_FREQ: f64 = 0.008;
const PEAKS_VALLEYS_FREQ: f64 = 0.03;
const FAVOR_FREQ: f64 = 0.003;

/// Gold vein noise frequency -- high frequency produces dense, thin veins.
const GOLD_VEIN_FREQ: f64 = 0.05;
/// Half-width of the noise band that counts as a vein.
/// The vein appears where `|noise| < threshold`, tracing the zero-contour
/// of the Perlin field. Smaller = thinner veins.
const GOLD_VEIN_THRESHOLD: f64 = 0.02;

/// Octave count for the biome noise layers.
const LAYER_OCTAVES: u32 = 5;
/// Amplitude decay per octave.
const LAYER_PERSISTENCE: f64 = 0.5;
/// Frequency multiplier per octave.
const LAYER_LACUNARITY: f64 = 2.0;

// ---------------------------------------------------------------------------
// Biome enum
// ---------------------------------------------------------------------------

/// Resource production modifier for a biome tile.
///
/// Positive values are bonuses, negative are maluses. Expressed as
/// percentage points (e.g. `+20` = +20 % production rate). A zero
/// means the biome has no effect on that resource.
///
/// Gold is intentionally absent -- it is **not** a passive resource.
/// Gold comes exclusively from rare active-farming nodes (see
/// [`Biome::has_gold_veins`]). Cities near gold-bearing biomes
/// may discover nodes, but gold is never a steady per-tile trickle.
///
/// # Resource types
///
/// | Resource | Source | Role |
/// |----------|--------|------|
/// | **Wood** | Passive logging camps | Construction, ships |
/// | **Stone** | Passive quarries | Fortifications, buildings |
/// | **Food** | Passive farms / fishing | Population, army upkeep |
/// | **Metal** | Passive mines | Weapons, armour, siege |
/// | **Favor** | Passive temples + rare biome boost | God powers, mythicals |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceModifiers {
    /// Passive logging bonus/malus (%).
    pub wood: i8,
    /// Passive quarry bonus/malus (%).
    pub stone: i8,
    /// Passive farm bonus/malus (%). Coastal cities gain fish here.
    pub food: i8,
    /// Passive mine bonus/malus (%).
    pub metal: i8,
    /// Passive temple & biome-granted Favor bonus/malus (%).
    pub favor: i8,
}

impl ResourceModifiers {
    const fn new(wood: i8, stone: i8, food: i8, metal: i8, favor: i8) -> Self {
        Self { wood, stone, food, metal, favor }
    }
}

/// Biome classification for each tile.
///
/// Each variant carries strategic / military significance in a
/// Grepolis-style game. See [`Biome::resource_modifiers`] for the
/// per-biome production bonuses and maluses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Biome {
    /// Open ocean -- no land resources; fishing food for adjacent coastal cities.
    Ocean = 0,
    /// Shallow coastal water -- fishing (food) for shore cities.
    Coast = 1,
    /// Sandy shore -- minor food from fishing, poor building materials.
    Beach = 2,
    /// Flat fertile land -- strong farms (food), decent logging (wood).
    Plains = 3,
    /// Dense woodland -- excellent logging (wood), good food (foraging).
    Forest = 4,
    /// Waterlogged lowland -- food from rice/reeds, Favor from sacred waters, poor stone.
    Swamp = 5,
    /// Rolling elevated terrain -- strong quarries (stone), moderate farms.
    Hills = 6,
    /// High rocky terrain -- rich mines (metal), excellent quarries (stone), poor farms.
    Mountains = 7,
    /// Frozen mountain tops -- deep metal veins, minor stone, very poor food.
    SnowyPeaks = 8,
    /// Hot arid land -- rare gold nodes (active), poor food/wood, some stone.
    Desert = 9,
    /// Frozen lowland -- metal deposits, poor food/wood, minor Favor from northern temples.
    Tundra = 10,
    /// Sheltered low point -- rare gold veins (active), good food (irrigated), decent wood.
    Valley = 11,
    /// Elevated plateau -- stone quarries, moderate metal, poor wood.
    Highlands = 12,
    /// Divinely attuned area -- massive Favor boost, good wood (ancient trees), no metal.
    SacredGrove = 13,
    /// Deep water -- enables deep harbors; rare sea-trade gold nodes (active).
    DeepHarbor = 14,
    /// Decorative terrain beyond the playable area.
    FarLand = 15,
}

impl Biome {
    /// Serialize to the `u8` stored in the binary format.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Deserialize from a `u8` read from chunk data.
    ///
    /// Unknown values fall back to [`Ocean`](Biome::Ocean).
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Biome::Ocean,
            1 => Biome::Coast,
            2 => Biome::Beach,
            3 => Biome::Plains,
            4 => Biome::Forest,
            5 => Biome::Swamp,
            6 => Biome::Hills,
            7 => Biome::Mountains,
            8 => Biome::SnowyPeaks,
            9 => Biome::Desert,
            10 => Biome::Tundra,
            11 => Biome::Valley,
            12 => Biome::Highlands,
            13 => Biome::SacredGrove,
            14 => Biome::DeepHarbor,
            15 => Biome::FarLand,
            _ => Biome::Ocean,
        }
    }

    /// Human-readable name for display / debugging.
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "Ocean",
            Biome::Coast => "Coast",
            Biome::Beach => "Beach",
            Biome::Plains => "Plains",
            Biome::Forest => "Forest",
            Biome::Swamp => "Swamp",
            Biome::Hills => "Hills",
            Biome::Mountains => "Mountains",
            Biome::SnowyPeaks => "Snowy Peaks",
            Biome::Desert => "Desert",
            Biome::Tundra => "Tundra",
            Biome::Valley => "Valley",
            Biome::Highlands => "Highlands",
            Biome::SacredGrove => "Sacred Grove",
            Biome::DeepHarbor => "Deep Harbor",
            Biome::FarLand => "Far Land",
        }
    }

    /// Per-biome **passive** resource production modifiers (percentage points).
    ///
    /// These represent the five passive resources: wood, stone, food, metal, favor.
    /// Gold is deliberately excluded -- see [`has_gold_veins`](Self::has_gold_veins).
    ///
    /// Because cities are placed on shores, nearby water tiles (Ocean, Coast,
    /// DeepHarbor) contribute food from fishing to the city's aggregate.
    ///
    /// **Favor** is intentionally low per-tile. The main Favor driver is
    /// *island size*: small islands (near the minimum player count) receive a
    /// large Favor multiplier during [`compute_city_resources`]. Biome tiles
    /// provide only a base that the island-size multiplier amplifies -- so a
    /// Sacred Grove on a tiny island is worth far more than one on a
    /// continent.
    ///
    /// Design rationale:
    /// - **Wood** comes from trees -> Forest, Valley, Sacred Grove, Plains.
    /// - **Stone** comes from quarries -> Hills, Mountains, Highlands, Desert.
    /// - **Food** comes from farms/fishing -> Plains, Valley, Swamp, Coast, Ocean.
    /// - **Metal** comes from mines -> Mountains, Snowy Peaks, Tundra, Hills.
    /// - **Favor** base from sacred/spiritual land -> Sacred Grove, Swamp, Tundra.
    pub fn resource_modifiers(self) -> ResourceModifiers {
        //                          wood  stone  food  metal  favor
        match self {
            // -- Water biomes (fishing food for adjacent shore cities) ----------
            Biome::Ocean      => ResourceModifiers::new(   0,    0,   10,    0,    0),
            Biome::Coast      => ResourceModifiers::new(   0,    0,   15,    0,    0),
            Biome::DeepHarbor => ResourceModifiers::new(   0,    0,    5,    0,    0),

            // -- Transitional --------------------------------------------------
            Biome::Beach      => ResourceModifiers::new( -10,  -10,   10,    0,    0),

            // -- Fertile lowlands ----------------------------------------------
            Biome::Plains     => ResourceModifiers::new(  10,    0,   25,    0,    2),
            Biome::Forest     => ResourceModifiers::new(  30,    0,   10,    0,    0),
            Biome::Swamp      => ResourceModifiers::new(   5,  -15,   15,    0,    8),
            Biome::Valley     => ResourceModifiers::new(  15,    0,   20,    0,    0),

            // -- Elevated terrain ----------------------------------------------
            Biome::Hills      => ResourceModifiers::new(   0,   25,   10,   10,    0),
            Biome::Mountains  => ResourceModifiers::new( -15,   20,  -15,   25,    0),
            Biome::SnowyPeaks => ResourceModifiers::new( -20,   10,  -25,   30,    0),
            Biome::Highlands  => ResourceModifiers::new( -10,   20,   -5,   15,    0),

            // -- Climate extremes ----------------------------------------------
            Biome::Desert     => ResourceModifiers::new( -20,   10,  -20,    0,    0),
            Biome::Tundra     => ResourceModifiers::new( -15,    5,  -15,   20,    5),

            // -- Rare / special ------------------------------------------------
            //    Sacred Grove: meaningful base Favor, amplified on small islands.
            Biome::SacredGrove=> ResourceModifiers::new(  15,    0,    5,  -20,   20),

            // -- Non-playable --------------------------------------------------
            Biome::FarLand    => ResourceModifiers::new(   0,    0,    0,    0,    0),
        }
    }

    /// Whether this biome can contain gold veins.
    ///
    /// Gold veins are traced by Perlin noise contours, but only appear in
    /// biomes that are geologically plausible for gold deposits.
    pub fn has_gold_veins(self) -> bool {
        matches!(
            self,
            Biome::Valley
                | Biome::Desert
                | Biome::DeepHarbor
                | Biome::Highlands
                | Biome::Mountains
                | Biome::SnowyPeaks
        )
    }
}

// ---------------------------------------------------------------------------
// Gold vein sampler
// ---------------------------------------------------------------------------

/// Samples a dedicated Perlin noise layer to locate gold veins.
///
/// Gold veins follow the **zero-contour** of the noise field: wherever
/// `|noise(x, y)| < GOLD_VEIN_THRESHOLD` the tile is on a vein. Because
/// Perlin contour lines are smooth and connected, this produces thin,
/// river-like veins that meander organically across the map.
///
/// Create once per world (requires the seed) and reuse for every query.
pub struct GoldVeinSampler {
    layer: NoiseLayer,
}

impl GoldVeinSampler {
    /// Build a sampler from a world seed.
    pub fn new(seed: u32) -> Self {
        let mut rng = StdRng::seed_from_u64(seed as u64 ^ 0x601D_AE17);
        let layer = NoiseLayer::new(
            seed.wrapping_mul(13).wrapping_add(7),
            GOLD_VEIN_FREQ,
            &mut rng,
        );
        Self { layer }
    }

    /// Returns `true` if the tile at `(x, y)` lies on a gold vein **and**
    /// the biome allows gold.
    pub fn is_gold(&self, x: usize, y: usize, biome: Biome) -> bool {
        if !biome.has_gold_veins() {
            return false;
        }
        let v = self.layer.sample(x, y);
        v.abs() < GOLD_VEIN_THRESHOLD
    }
}

/// A single noise layer with its own Perlin instance, offsets, and frequency.
struct NoiseLayer {
    perlin: Perlin,
    offset_x: f64,
    offset_y: f64,
    base_freq: f64,
}

impl NoiseLayer {
    /// Create a new layer with a unique seed and random offsets.
    fn new(seed: u32, base_freq: f64, rng: &mut StdRng) -> Self {
        Self {
            perlin: Perlin::new(seed),
            offset_x: (rng.random::<u32>() % 10_000) as f64,
            offset_y: (rng.random::<u32>() % 10_000) as f64,
            base_freq,
        }
    }

    /// Sample the layer at world coordinate `(x, y)` using octave blending.
    ///
    /// Returns a value in approximately `[-1, 1]`.
    fn sample(&self, x: usize, y: usize) -> f64 {
        let mut value = 0.0;
        let mut amp = 1.0;
        let mut amp_sum = 0.0;
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
// Generation entry point
// ---------------------------------------------------------------------------

/// Generate biome classifications for every tile.
///
/// Reuses the existing `elevation` grid (the "Elevation" layer) and
/// generates 5 additional noise layers (continentalness, erosion,
/// temperature, peaks/valleys, favor harmony). All layers are sampled
/// at each tile and combined via threshold rules to produce a [`Biome`].
pub fn generate_biomes(
    config: &WorldConfig,
    terrain: &[Vec<Terrain>],
    elevation: &[Vec<f64>],
) -> Vec<Vec<u8>> {
    let size = config.map_len();

    // Derive unique seeds for each layer from the world seed.
    let base = config.seed;
    let mut rng = StdRng::seed_from_u64(base as u64 ^ 0xB10_E5EED);

    let continentalness = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(1), CONTINENTALNESS_FREQ, &mut rng);
    let erosion         = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(2), EROSION_FREQ, &mut rng);
    let temperature     = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(3), TEMPERATURE_FREQ, &mut rng);
    let peaks_valleys   = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(4), PEAKS_VALLEYS_FREQ, &mut rng);
    let favor           = NoiseLayer::new(base.wrapping_mul(7).wrapping_add(5), FAVOR_FREQ, &mut rng);

    let wt = config.water_threshold as f64;

    let mut biomes = vec![vec![0u8; size]; size];

    for y in 0..size {
        for x in 0..size {
            let t = terrain[y][x];
            let e = elevation[y][x];

            let c = continentalness.sample(x, y);
            let er = erosion.sample(x, y);
            let tp = temperature.sample(x, y);
            let pv = peaks_valleys.sample(x, y);
            let fv = favor.sample(x, y);

            let biome = match t {
                Terrain::Water => classify_water(e, wt, c),
                Terrain::Land => classify_land(e, wt, c, er, tp, pv, fv),
                Terrain::FarLand => Biome::FarLand,
            };

            biomes[y][x] = biome.to_u8();
        }
    }

    biomes
}

// ---------------------------------------------------------------------------
// Classification rules
// ---------------------------------------------------------------------------

/// Classify a water tile.
///
/// * Very low continentalness -> [`DeepHarbor`](Biome::DeepHarbor) (deep
///   water, ship speed bonus).
/// * Close to the land threshold -> [`Coast`](Biome::Coast).
/// * Otherwise -> [`Ocean`](Biome::Ocean).
fn classify_water(elev: f64, water_threshold: f64, continentalness: f64) -> Biome {
    // Deep harbors: low continental value = deep ocean basins
    if continentalness < -0.35 {
        return Biome::DeepHarbor;
    }
    // Coast: elevation close to the land/water boundary
    if elev > water_threshold - 0.06 {
        return Biome::Coast;
    }
    Biome::Ocean
}

/// Classify a land tile using all noise layers.
///
/// Priority order (most specific / rarest first):
///
/// 1. Beach -- barely above water.
/// 2. Sacred Grove -- high Favor harmony (rare).
/// 3. Snowy Peaks -- very high elevation + cold.
/// 4. Mountains -- high elevation.
/// 5. Desert -- hot + eroded.
/// 6. Tundra -- cold.
/// 7. Highlands -- peaks in noise + elevated.
/// 8. Valley -- valleys in noise.
/// 9. Swamp -- wet + low.
/// 10. Forest -- low erosion.
/// 11. Hills -- moderate elevation.
/// 12. Plains -- default.
fn classify_land(
    elev: f64,
    water_threshold: f64,
    _continentalness: f64,
    erosion: f64,
    temperature: f64,
    peaks_valleys: f64,
    favor: f64,
) -> Biome {
    let above_water = elev - water_threshold;

    // Beach: barely above water
    if above_water < 0.02 {
        return Biome::Beach;
    }

    // Sacred Grove: high divine attunement (rare)
    if favor > 0.55 {
        return Biome::SacredGrove;
    }

    // Snowy Peaks: very high elevation + cold temperature
    if above_water > 0.28 && temperature < -0.15 {
        return Biome::SnowyPeaks;
    }

    // Mountains: high elevation
    if above_water > 0.22 {
        return Biome::Mountains;
    }

    // Desert: hot + eroded
    if temperature > 0.35 && erosion > 0.15 {
        return Biome::Desert;
    }

    // Tundra: cold
    if temperature < -0.30 {
        return Biome::Tundra;
    }

    // Highlands: peaks in noise + moderately elevated
    if peaks_valleys > 0.45 && above_water > 0.08 {
        return Biome::Highlands;
    }

    // Valley: valleys in noise (sheltered low points)
    if peaks_valleys < -0.40 {
        return Biome::Valley;
    }

    // Swamp: very wet + low elevation
    if erosion < -0.30 && above_water < 0.08 {
        return Biome::Swamp;
    }

    // Forest: low erosion = dense vegetation
    if erosion < -0.05 {
        return Biome::Forest;
    }

    // Hills: moderate elevation above water
    if above_water > 0.10 {
        return Biome::Hills;
    }

    // Default: fertile plains
    Biome::Plains
}

// ---------------------------------------------------------------------------
// Per-city resource aggregation
// ---------------------------------------------------------------------------

/// Radius (in tiles) around a city center used to scan biome tiles.
///
/// Cities sit on shores, so half the scanned area will be water
/// (contributing fishing food) and half land. A radius of 6 gives a
/// ~113-tile circle -- roughly the farmable hinterland of a single city.
pub const CITY_SCAN_RADIUS: i32 = 6;

/// Aggregated resource profile for a single city slot.
///
/// Computed once during world generation by scanning all tiles within
/// [`CITY_SCAN_RADIUS`] of the city center. Stored in the world file
/// so the game can display resource previews without recalculating.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CityResources {
    /// Aggregate passive wood modifier (sum of tile bonuses / tile count).
    pub wood: i16,
    /// Aggregate passive stone modifier.
    pub stone: i16,
    /// Aggregate passive food modifier (includes fishing from water tiles).
    pub food: i16,
    /// Aggregate passive metal modifier.
    pub metal: i16,
    /// Aggregate passive Favor modifier.
    pub favor: i16,
    /// Number of gold-vein tiles within the city's radius.
    ///
    /// Gold is **never** passive -- each node must be actively farmed.
    /// This count tells the player how many farmable gold sources are
    /// available, not a production rate.
    pub gold_nodes: u8,
    /// Dominant biome within the scanned radius (for UI display).
    pub dominant_biome: u8,
}

/// Compute [`CityResources`] for every city slot.
///
/// For each city at `(cx, cy)`, samples all tiles within
/// [`CITY_SCAN_RADIUS`] and:
/// 1. Accumulates `ResourceModifiers` from each tile's biome.
/// 2. Counts gold-vein tiles using [`GoldVeinSampler`] -- gold appears
///    along thin, river-like Perlin noise contours in eligible biomes.
/// 3. Identifies the dominant biome by tile count.
/// 4. Applies an **island-size Favor multiplier**: small islands (at or
///    near `min_cities_per_island`) get a large Favor bonus, while big
///    islands get almost none. This makes Sacred Grove + tiny island the
///    strongest Favor combination in the game.
///
/// # Island-size Favor formula
///
/// ```text
/// ratio = min_cities / island_cities   (1.0 for minimum-size islands)
/// multiplier = 1.0 + 2.0 * ratio^2     (3.0* at minimum, ~1.1* for huge)
/// favor = base_favor * multiplier
/// ```
///
/// Returns a `Vec` parallel to `city_slots`.
pub fn compute_city_resources(
    city_slots: &[(usize, usize)],
    biomes: &[Vec<u8>],
    region_labels: &[Vec<usize>],
    region_city_counts: &std::collections::HashMap<usize, u32>,
    min_cities_per_island: u32,
    seed: u32,
) -> Vec<CityResources> {
    let map_h = biomes.len();
    let map_w = if map_h > 0 { biomes[0].len() } else { 0 };
    let r = CITY_SCAN_RADIUS;
    let min_f = min_cities_per_island.max(1) as f64;
    let gold_sampler = GoldVeinSampler::new(seed);

    city_slots
        .iter()
        .map(|&(cx, cy)| {
            let mut wood_sum: i32 = 0;
            let mut stone_sum: i32 = 0;
            let mut food_sum: i32 = 0;
            let mut metal_sum: i32 = 0;
            let mut favor_sum: i32 = 0;
            let mut gold_nodes: u8 = 0;
            let mut tile_count: u32 = 0;
            let mut biome_counts = [0u32; 16];

            for dy in -r..=r {
                for dx in -r..=r {
                    // Circular scan
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                    let tx = cx as i32 + dx;
                    let ty = cy as i32 + dy;
                    if tx < 0 || ty < 0 || tx >= map_w as i32 || ty >= map_h as i32 {
                        continue;
                    }
                    let (tx, ty) = (tx as usize, ty as usize);

                    let biome = Biome::from_u8(biomes[ty][tx]);
                    let mods = biome.resource_modifiers();

                    wood_sum += mods.wood as i32;
                    stone_sum += mods.stone as i32;
                    food_sum += mods.food as i32;
                    metal_sum += mods.metal as i32;
                    favor_sum += mods.favor as i32;
                    tile_count += 1;

                    let b = biome.to_u8() as usize;
                    if b < 16 {
                        biome_counts[b] += 1;
                    }

                    // Gold vein check via Perlin noise contour
                    if gold_sampler.is_gold(tx, ty, biome) {
                        gold_nodes = gold_nodes.saturating_add(1);
                    }
                }
            }

            // Normalize: average modifier per tile, then clamp to i16
            let n = tile_count.max(1) as i32;
            let dominant = biome_counts
                .iter()
                .enumerate()
                .max_by_key(|&(_, &c)| c)
                .map(|(i, _)| i as u8)
                .unwrap_or(0);

            // --- Island-size Favor multiplier ---
            // Look up this city's island (region) and its city count.
            let region_id = region_labels[cy][cx];
            let island_cities = region_city_counts
                .get(&region_id)
                .copied()
                .unwrap_or(min_cities_per_island) as f64;
            // ratio = 1.0 for minimum-size islands, <1.0 for bigger ones
            let ratio = (min_f / island_cities).min(1.0);
            // multiplier: 3* at minimum size, tapering down for larger islands
            let favor_multiplier = 1.0 + 2.0 * ratio * ratio;
            let raw_favor = (favor_sum / n) as f64;
            let scaled_favor = (raw_favor * favor_multiplier).round();

            CityResources {
                wood: (wood_sum / n).clamp(-3200, 3200) as i16,
                stone: (stone_sum / n).clamp(-3200, 3200) as i16,
                food: (food_sum / n).clamp(-3200, 3200) as i16,
                metal: (metal_sum / n).clamp(-3200, 3200) as i16,
                favor: (scaled_favor as i32).clamp(-3200, 3200) as i16,
                gold_nodes,
                dominant_biome: dominant,
            }
        })
        .collect()
}
