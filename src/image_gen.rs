use std::fs;
use std::sync::Arc;
use std::collections::HashMap;
use image::{RgbImage, Rgb};
use rayon::prelude::*;
use crate::terrain::Terrain;
use crate::{MAP_SIZE, CHUNK_SIZE, CHUNK_FOLDER, CITY_SLOT_COLOR};

pub fn get_color(terrain: Terrain, elevation: f64) -> Rgb<u8> {
    match terrain {
        Terrain::Water => {
            let t_raw = ((elevation - 0.2) / (crate::WATER - 0.2)).clamp(0.0, 1.0);
            let t = t_raw.powf(0.5); // Slower transition to deep water
            let r = ((1.0 - t) * 17.0 + t * 70.0) as u8;
            let g = ((1.0 - t) * 55.0 + t * (150.0 + elevation * 80.0)) as u8;
            let b = ((1.0 - t) * 105.0 + t * (180.0 + elevation * 60.0)) as u8;
            Rgb([r, g, b])
        }
        Terrain::Land => {
            if elevation < crate::WATER + 0.02 {
                Rgb([229, 216, 176])
            } else if elevation < crate::WATER + 0.15 {
                let green_base = 120.0 + elevation * 100.0;
                let green_var = (30.0 * (elevation * 20.0).sin()) as i32;
                let r = (40.0 + elevation * 60.0) as u8;
                let g = (green_base as i32 + green_var).clamp(0, 255) as u8;
                let b = (30.0 + elevation * 40.0) as u8;
                Rgb([r, g, b])
            } else if elevation < 0.75 {
                let t = (elevation - (crate::WATER + 0.1)) / (0.7 - (crate::WATER + 0.1));
                let r = ((1.0 - t) * 80.0 + t * 140.0) as u8;
                let g = ((1.0 - t) * 100.0 + t * 110.0) as u8;
                let b = ((1.0 - t) * 60.0 + t * 90.0) as u8;
                Rgb([r, g, b])
            } else {
                let t = ((elevation - 0.85) / 0.15).clamp(0.0, 1.0);
                let r = ((1.0 - t) * 180.0 + t * 240.0) as u8;
                let g = ((1.0 - t) * 180.0 + t * 240.0) as u8;
                let b = ((1.0 - t) * 190.0 + t * 255.0) as u8;
                Rgb([r, g, b])
            }
        }
    }
}

pub fn draw_city_circle(img: &mut RgbImage, cx: usize, cy: usize, color: Rgb<u8>) {
    let radius = 3i32;
    let (width, height) = (img.width() as i32, img.height() as i32);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let px = cx as i32 + dx;
                let py = cy as i32 + dy;
                if px >= 0 && px < width && py >= 0 && py < height {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

pub fn generate_image_chunks(
    terrain: &Vec<Vec<Terrain>>,
    elevation: &Vec<Vec<f64>>,
    city_slots: &Vec<(usize, usize)>
) {
    fs::create_dir_all(CHUNK_FOLDER).expect("Failed to create chunk folder");

    // Group cities by chunk coordinate
    let mut city_by_chunk: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for &(x, y) in city_slots {
        let chunk_x = x / CHUNK_SIZE;
        let chunk_y = y / CHUNK_SIZE;
        city_by_chunk.entry((chunk_x, chunk_y)).or_default().push((x, y));
    }
    let city_by_chunk = Arc::new(city_by_chunk);

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

            // Draw city circles
            if let Some(cities) = city_by_chunk.get(&(cx, cy)) {
                for &(city_x, city_y) in cities {
                    draw_city_circle(&mut img, city_x - x0, city_y - y0, CITY_SLOT_COLOR);
                }
            }

            let filename = format!("{}/chunk_{}_{}.png", CHUNK_FOLDER, cx, cy);
            img.save(&filename).expect("Failed to save image chunk");
        }
    });
}
