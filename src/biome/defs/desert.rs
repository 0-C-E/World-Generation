use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Desert",
    modifiers: ResourceModifiers::new(-20, 20, -25, 15, 10),
    has_gold_veins: true,
    land_color: Some(|t| {
        [
            (185.0 + t * 40.0) as u8,
            (160.0 + t * 40.0) as u8,
            (95.0 + t * 45.0) as u8,
        ]
    }),
    water_color: None,
};
