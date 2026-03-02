use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Mountains",
    modifiers: ResourceModifiers::new(-15, 20, -15, 25, 0),
    has_gold_veins: true,
    land_color: Some(|t| {
        [
            (115.0 + t * 50.0) as u8,
            (105.0 + t * 45.0) as u8,
            (90.0 + t * 45.0) as u8,
        ]
    }),
    water_color: None,
};
