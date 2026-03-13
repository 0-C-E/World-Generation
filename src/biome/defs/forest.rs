use crate::biome::{BiomeData, ResourceModifiers};

pub const DATA: BiomeData = BiomeData {
    name: "Forest",
    modifiers: ResourceModifiers::new(30, -5, 10, -20, -15),
    has_gold_veins: false,
    // Sinusoidal elevation variation gives a canopy texture.
    land_color: Some(|t| {
        // `t` is in [0,1] over the land elevation range; reconstruct raw `e`
        // for the sin term (approximate — good enough for visual texture).
        let e_approx = 0.5 + t * 0.5;
        let v = (12.0 * (e_approx * 30.0).sin()) as i32;
        [
            (20.0 + t * 40.0) as u8,
            ((80.0 + t * 45.0) as i32 + v).clamp(55, 145) as u8,
            (15.0 + t * 30.0) as u8,
        ]
    }),
    water_color: None,
};
