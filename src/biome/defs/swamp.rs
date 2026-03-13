use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Swamp",
    modifiers: ResourceModifiers::new(5, -20, 15, -10, 10),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (45.0 + t * 40.0) as u8,
            (65.0 + t * 40.0) as u8,
            (35.0 + t * 30.0) as u8,
        ]
    }),
    water_color: None,
};
