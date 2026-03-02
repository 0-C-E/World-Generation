use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Beach",
    modifiers: ResourceModifiers::new(-10, -10, 10, 0, 0),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (210.0 + t * 25.0) as u8,
            (195.0 + t * 25.0) as u8,
            (150.0 + t * 35.0) as u8,
        ]
    }),
    water_color: None,
};
