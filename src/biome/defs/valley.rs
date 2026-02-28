use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Valley",
    modifiers: ResourceModifiers::new(15, 0, 20, 0, 0),
    has_gold_veins: true,
    land_color: Some(|t| {
        [
            (55.0 + t * 55.0) as u8,
            (130.0 + t * 45.0) as u8,
            (35.0 + t * 40.0) as u8,
        ]
    }),
    water_color: None,
};
