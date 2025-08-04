use std::fs;
use image::{RgbImage, Rgb};
use rayon::prelude::*;

use crate::config::{MAP_SIZE, CHUNK_SIZE, WATER};
use crate::terrain::Terrain;
use crate::CHUNK_FOLDER;

pub fn get_color(terrain: Terrain, elevation: f64) -> Rgb<u8> {
    match terrain {
        Terrain::Water => {
            let t_raw = ((elevation - 0.2) / (WATER - 0.2)).clamp(0.0, 1.0);
            let t = t_raw.powf(0.65); // Slower transition to deep water
            let r = ((1.0 - t) * 17.0 + t * 70.0) as u8;
            let g = ((1.0 - t) * 55.0 + t * (150.0 + elevation * 80.0)) as u8;
            let b = ((1.0 - t) * 105.0 + t * (180.0 + elevation * 60.0)) as u8;
            Rgb([r, g, b])
        }
        Terrain::Land => {
            // Normalize elevation for land
            let land_elevation = ((elevation - WATER) / (1.0 - WATER)).clamp(0.0, 1.0);

            if land_elevation < 0.1 {
                // Beach/coastal areas - sandy colors
                let t = land_elevation / 0.1;
                let beach_base = [240, 220, 180]; // Light sand
                let coastal_green = [200, 210, 160]; // Coastal vegetation
                blend_colors(beach_base, coastal_green, t)
            } else if land_elevation < 0.4 {
                // Lowlands - grasslands and light forests
                let t = (land_elevation - 0.1) / 0.3;
                let light_green = [120, 180, 90];   // Grasslands
                let forest_green = [80, 140, 70];   // Light forest

                // Add subtle noise for natural variation
                let noise_factor = (elevation * 50.0).sin() * 0.1;
                let varied_t = (t + noise_factor).clamp(0.0, 1.0);

                blend_colors(light_green, forest_green, varied_t)
            } else if land_elevation < 0.7 {
                // Hills - mixed forest and rocky areas
                let t = (land_elevation - 0.4) / 0.3;
                let dark_forest = [60, 100, 50];    // Dark forest
                let rocky_brown = [100, 80, 60];    // Rocky terrain

                // Vary between forest and rock based on elevation detail
                let rock_factor = ((elevation * 30.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                let base_color = blend_colors(dark_forest, rocky_brown, rock_factor);

                // Blend with higher elevation colors
                let mountain_gray = [120, 110, 90];
                blend_rgb_colors(base_color, mountain_gray, t * 0.3)
            } else if land_elevation < 0.9 {
                // Mountains - rocky grays and browns
                let t = (land_elevation - 0.7) / 0.2;
                let mountain_brown = [110, 90, 70];
                let mountain_gray = [130, 120, 100];
                blend_colors(mountain_brown, mountain_gray, t)
            } else {
                // High peaks - snow and rock
                let t = (land_elevation - 0.9) / 0.1;
                let dark_rock = [100, 95, 85];
                let snow = [240, 240, 245];
                blend_colors(dark_rock, snow, t.powf(0.5)) // Gradual snow transition
            }
        }
        Terrain::FarLand => {
            let t = (elevation - 0.8) / 0.2;
            let r = (200.0 + t * 55.0) as u8;
            let g = (180.0 + t * 75.0) as u8;
            let b = (160.0 + t * 95.0) as u8;
            Rgb([r, g, b])
        }
    }
}

fn blend_colors(color1: [u8; 3], color2: [u8; 3], t: f64) -> Rgb<u8> {
    let t = t.clamp(0.0, 1.0);
    let r = ((1.0 - t) * color1[0] as f64 + t * color2[0] as f64) as u8;
    let g = ((1.0 - t) * color1[1] as f64 + t * color2[1] as f64) as u8;
    let b = ((1.0 - t) * color1[2] as f64 + t * color2[2] as f64) as u8;
    Rgb([r, g, b])
}

fn blend_rgb_colors(color1: Rgb<u8>, color2: [u8; 3], t: f64) -> Rgb<u8> {
    blend_colors([color1[0], color1[1], color1[2]], color2, t)
}

pub fn generate_image_chunks(
    terrain: &Vec<Vec<Terrain>>,
    elevation: &Vec<Vec<f64>>,
) {
    fs::create_dir_all(CHUNK_FOLDER).expect("Failed to create chunk folder");

    let num_chunks_x = MAP_SIZE / CHUNK_SIZE;
    let num_chunks_y = MAP_SIZE / CHUNK_SIZE;

    // Parallel chunk generation using rayon
    (0..num_chunks_y).into_par_iter().for_each(|cy| {
        for cx in 0..num_chunks_x {
            let x0 = cx * CHUNK_SIZE;
            let y0 = cy * CHUNK_SIZE;

            let mut img = RgbImage::new(CHUNK_SIZE as u32, CHUNK_SIZE as u32);

            for iy in 0..CHUNK_SIZE {
                for ix in 0..CHUNK_SIZE {
                    let global_x = x0 + ix;
                    let global_y = y0 + iy;
                    let terrain_val = terrain[global_y][global_x];
                    let elevation_val = elevation[global_y][global_x];
                    let color = get_color(terrain_val, elevation_val);
                    img.put_pixel(ix as u32, iy as u32, color);
                }
            }

            let filename = format!("{}/chunk_{}_{}.png", CHUNK_FOLDER, cx, cy);
            img.save(&filename).expect("Failed to save image chunk");
        }
    });
}
