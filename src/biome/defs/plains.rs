use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Plains",
    modifiers: ResourceModifiers::new(5, -10, 25, -15, -5),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (75.0 + t * 80.0) as u8,
            (130.0 + t * 65.0) as u8,
            (30.0 + t * 60.0) as u8,
        ]
    }),
    water_color: None,
};
