use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Tundra",
    modifiers: ResourceModifiers::new(-15, 5, -15, 20, 5),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (95.0 + t * 100.0) as u8,
            (110.0 + t * 90.0) as u8,
            (135.0 + t * 75.0) as u8,
        ]
    }),
    water_color: None,
};
