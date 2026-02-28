use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Coast",
    modifiers: ResourceModifiers::new(0, 0, 15, 0, 0),
    has_gold_veins: false,
    land_color: None,
    water_color: Some(|e, wt| {
        let t = ((e - 0.18) / (wt - 0.18)).clamp(0.0, 1.0).powf(0.7);
        [
            (40.0 + t * 70.0) as u8,
            (110.0 + t * 90.0) as u8,
            (150.0 + t * 60.0) as u8,
        ]
    }),
};
