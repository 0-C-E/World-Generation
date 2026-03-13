use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Ocean",
    modifiers: ResourceModifiers::new(0, 0, 15, 0, -15),
    has_gold_veins: false,
    land_color: None,
    water_color: Some(|e, wt| {
        let t = ((e - 0.10) / (wt - 0.10)).clamp(0.0, 1.0).powf(0.7);
        [
            ((1.0 - t) * 10.0 + t * 80.0) as u8,
            ((1.0 - t) * 35.0 + t * (170.0 + e * 60.0)) as u8,
            ((1.0 - t) * 80.0 + t * (200.0 + e * 60.0)) as u8,
        ]
    }),
};
