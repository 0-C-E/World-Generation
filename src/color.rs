//! Elevation-to-colour mapping for the tile renderer.
//!
//! Each tile is coloured by [`Terrain`] type and elevation: deep ocean blues
//! through sandy beaches and forests up to snowy mountain peaks.

use crate::terrain::Terrain;

/// Map a terrain type and elevation to an RGB colour triple.
///
/// * [`Water`](Terrain::Water) -- deep blue to lighter cyan
/// * [`Land`](Terrain::Land) -- beach sand to forest to grassland to snow
/// * [`FarLand`](Terrain::FarLand) -- washed-out beige beyond the playable area
///
/// `elevation` is in `[0.0, 1.0]`.
pub fn get_color(terrain: Terrain, elevation: f32, water_threshold: f32) -> [u8; 3] {
    let e = elevation;

    match terrain {
        // -- Water: deep blue -> lighter cyan --------------------------------
        Terrain::Water => {
            let t = ((e - 0.2) / (water_threshold - 0.2)).clamp(0.0, 1.0).powf(0.5);
            let r = ((1.0 - t) * 17.0 + t * 70.0) as u8;
            let g = ((1.0 - t) * 55.0 + t * (150.0 + e * 80.0)) as u8;
            let b = ((1.0 - t) * 105.0 + t * (180.0 + e * 60.0)) as u8;
            [r, g, b]
        }

        // -- Land: beach -> forest -> grassland -> mountain/snow ---------------
        Terrain::Land => {
            if e < water_threshold + 0.02 {
                // Beach sand
                [229, 216, 176]
            } else if e < water_threshold + 0.15 {
                // Dense forest
                let green_base = 120.0 + e * 100.0;
                let green_var = (30.0 * (e * 20.0).sin()) as i32;
                let r = (40.0 + e * 60.0) as u8;
                let g = (green_base as i32 + green_var).clamp(0, 255) as u8;
                let b = (30.0 + e * 40.0) as u8;
                [r, g, b]
            } else if e < 0.75 {
                // Grassland -> rocky foothills
                let t = (e - (water_threshold + 0.1)) / (0.7 - (water_threshold + 0.1));
                let r = ((1.0 - t) * 80.0 + t * 140.0) as u8;
                let g = ((1.0 - t) * 100.0 + t * 110.0) as u8;
                let b = ((1.0 - t) * 60.0 + t * 90.0) as u8;
                [r, g, b]
            } else {
                // Mountain -> snow
                let t = ((e - 0.85) / 0.15).clamp(0.0, 1.0);
                let r = ((1.0 - t) * 180.0 + t * 240.0) as u8;
                let g = ((1.0 - t) * 180.0 + t * 240.0) as u8;
                let b = ((1.0 - t) * 190.0 + t * 255.0) as u8;
                [r, g, b]
            }
        }

        // -- FarLand: washed-out beige beyond world edge --------------------
        Terrain::FarLand => {
            let t = (e - 0.8) / 0.2;
            let r = (200.0 + t * 55.0) as u8;
            let g = (180.0 + t * 75.0) as u8;
            let b = (160.0 + t * 95.0) as u8;
            [r, g, b]
        }

    }
}
