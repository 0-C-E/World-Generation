use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Hills",
    modifiers: ResourceModifiers::new(-5, 25, 5, 5, -30),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (100.0 + t * 55.0) as u8,
            (110.0 + t * 40.0) as u8,
            (60.0 + t * 45.0) as u8,
        ]
    }),
    water_color: None,
};
