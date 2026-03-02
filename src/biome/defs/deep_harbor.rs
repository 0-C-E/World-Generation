use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Deep Harbor",
    modifiers: ResourceModifiers::new(0, 0, 5, 0, 0),
    has_gold_veins: true,
    land_color: None,
    water_color: Some(|e, wt| {
        let t = (e / wt).clamp(0.0, 1.0);
        [
            (10.0 + t * 15.0) as u8,
            (25.0 + t * 45.0) as u8,
            (70.0 + t * 50.0) as u8,
        ]
    }),
};
