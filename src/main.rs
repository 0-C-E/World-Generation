use std::time::Instant;
use noise::{NoiseFn, Perlin};

mod city;
mod image_gen;
mod save;
mod terrain;

use city::{find_city_slots, filter_city_slots_by_region};
use image_gen::generate_image_chunks;
use save::{WorldSave, IslandInfo, CityInfo};
use std::fs::File;
use std::io::Write;

// Constants
const MAP_SIZE: usize = 5_000;
const SCALE: f64 = 40.0;
const OCTAVES: usize = 6;
const PERSISTENCE: f64 = 0.5;
const LACUNARITY: f64 = 2.5;

const WATER: f64 = 0.55;

const CITY_SPACING: usize = 5;
const MIN_CITY_SLOTS_PER_ISLAND: usize = 6;
const CITY_RADIUS: f64 = (MAP_SIZE as f64 / 2.0) * 0.8;
const CHUNK_SIZE: usize = 250;
const CHUNK_FOLDER: &str = "chunks";

const CITY_SLOT_COLOR: image::Rgb<u8> = image::Rgb([100, 100, 100]);

fn main() {
    let seed = rand::random::<u32>();
    let start_time = Instant::now();
    
    println!("Generating elevation with seed {}...", seed);
    let mut step_time = Instant::now();
    let elevation = generate_elevation(seed);
    println!("Elevation took {:.2?}", step_time.elapsed());
    
    println!("Classifying terrain...");
    step_time = Instant::now();
    let terrain = terrain::classify_terrain(&elevation);
    println!("Terrain classification took {:.2?}", step_time.elapsed());
    
    println!("Labeling regions...");
    step_time = Instant::now();
    let region_map = terrain::label_regions(&terrain);
    println!("Region labeling took {:.2?}", step_time.elapsed());
    
    println!("Finding city slots...");
    step_time = Instant::now();
    let city_slots = find_city_slots(&terrain);
    println!("City slots finding took {:.2?}", step_time.elapsed());
    
    println!("Filtering city slots by region...");
    step_time = Instant::now();
    let filtered_city_slots = filter_city_slots_by_region(&city_slots, &region_map, MIN_CITY_SLOTS_PER_ISLAND);
    println!("City slots filtering took {:.2?}", step_time.elapsed());
    
    println!("City slots after filtering: {}", filtered_city_slots.len());
    step_time = Instant::now();
    if filtered_city_slots.is_empty() {
        println!("No valid city slots found, exiting.");
        return;
    }
    println!("City slots filtering took {:.2?}", step_time.elapsed());
    
    println!("Generating image chunks...");
    step_time = Instant::now();
    generate_image_chunks(&terrain, &elevation, &filtered_city_slots);
    println!("Image generation took {:.2?}", step_time.elapsed());

    // Save world data to file
    println!("Saving world data...");
    step_time = Instant::now();
    let mut islands = Vec::new();
    let mut region_tiles: std::collections::HashMap<usize, Vec<(usize, usize)>> = std::collections::HashMap::new();

    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let region_id = region_map[y][x];
            if region_id > 0 {
                region_tiles.entry(region_id).or_default().push((x, y));
            }
        }
    }
    println!("Region labeling took {:.2?}", step_time.elapsed());

    step_time = Instant::now();
    for (&region_id, tiles) in &region_tiles {
        let city_slots: Vec<CityInfo> = filtered_city_slots.iter()
            .filter(|&&(x, y)| region_map[y][x] == region_id)
            .map(|&(x, y)| CityInfo { x, y, region_id })
            .collect();
        if !city_slots.is_empty() {
            islands.push(IslandInfo {
                region_id,
                tiles: tiles.clone(),
                city_slots,
                size: tiles.len(),
            });
        }
    }
    println!("Region processing took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    let world_save = WorldSave { islands };

    // Save to file
    println!("Saving world data to file...");
    let json = serde_json::to_string_pretty(&world_save).unwrap();
    let mut file = File::create("world_save.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    println!("World data saved to world_save.json");
    println!("Saving to file took {:.2?}", step_time.elapsed());

    println!("Total generation time: {:.2?}", start_time.elapsed());
}

/// Step 1: Generate elevation using Perlin noise (multi-octave)
fn generate_elevation(seed: u32) -> Vec<Vec<f64>> {
    let perlin = Perlin::new(seed);

    let mut elevation = vec![vec![0.0; MAP_SIZE]; MAP_SIZE];
    let offset_x = rand::random::<u32>() % 10_000;
    let offset_y = rand::random::<u32>() % 10_000;

    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let mut freq = 1.0 / SCALE;
            let mut amp = 1.0;
            let mut noise_sum = 0.0;
            let mut amp_sum = 0.0;

            for _oct in 0..OCTAVES {
                let nx = (x as f64 + offset_x as f64) * freq;
                let ny = (y as f64 + offset_y as f64) * freq;
                noise_sum += perlin.get([nx, ny]) * amp;
                amp_sum += amp;
                amp *= PERSISTENCE;
                freq *= LACUNARITY;
            }
            elevation[y][x] = (noise_sum / amp_sum + 1.0) / 2.0;
        }
    }
    elevation
}
