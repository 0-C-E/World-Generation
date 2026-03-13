use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Sacred Grove",
    modifiers: ResourceModifiers::new(10, -20, 5, -25, 30),
    has_gold_veins: false,
    land_color: Some(|t| {
        [
            (15.0 + t * 65.0) as u8,
            (85.0 + t * 70.0) as u8,
            (50.0 + t * 60.0) as u8,
        ]
    }),
    water_color: None,
};
