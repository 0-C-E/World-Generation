use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Highlands",
    modifiers: ResourceModifiers::new(-15, 20, -10, 20, -15),
    has_gold_veins: true,
    land_color: Some(|t| {
        [
            (100.0 + t * 75.0) as u8,
            (90.0 + t * 70.0) as u8,
            (70.0 + t * 70.0) as u8,
        ]
    }),
    water_color: None,
};
