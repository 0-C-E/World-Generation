use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Snowy Peaks",
    modifiers: ResourceModifiers::new(-20, 10, -25, 30, 0),
    has_gold_veins: true,
    land_color: Some(|t| {
        [
            (170.0 + t * 70.0) as u8,
            (175.0 + t * 65.0) as u8,
            (190.0 + t * 65.0) as u8,
        ]
    }),
    water_color: None,
};
