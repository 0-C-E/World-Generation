use crate::biome::{BiomeData, ResourceModifiers};

// FarLand tiles are rendered by `Biome::get_color` using raw elevation
// regardless of `land_color`, so the function pointer here is never called.
// It exists only to satisfy the type — `None` would also work, but having
// an explicit colour makes the intent clearer if the fallback is ever hit.
pub const DATA: BiomeData = BiomeData {
    name: "Far Land",
    modifiers: ResourceModifiers::new(0, 0, 0, 0, 0),
    has_gold_veins: false,
    land_color: Some(|t| {
        // Washed-out beige — matches the `Terrain::FarLand` branch in
        // `Biome::get_color`, but using the land-rescaled `t` here.
        [
            (200.0 + t * 55.0) as u8,
            (180.0 + t * 75.0) as u8,
            (160.0 + t * 95.0) as u8,
        ]
    }),
    water_color: None,
};
