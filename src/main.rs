use std::time::Instant;
use noise::{NoiseFn, Perlin};

mod terrain;
mod city;
mod image_gen;

use city::{find_city_slots, filter_city_slots_by_region};
use image_gen::generate_image_chunks;

// Constants
const MAP_SIZE: usize = 5_000;
const SCALE: f64 = 40.0;
const OCTAVES: usize = 6;
const PERSISTENCE: f64 = 0.5;
const LACUNARITY: f64 = 2.5;

const WATER: f64 = 0.55;

const CITY_SPACING: usize = 5;
const MIN_CITY_SLOTS_PER_ISLAND: usize = 6;

const CHUNK_SIZE: usize = 250;
const CHUNK_FOLDER: &str = "chunks";

const CITY_SLOT_COLOR: image::Rgb<u8> = image::Rgb([100, 100, 100]);

fn main() {
    let seed = rand::random::<u32>();
    let start_time = Instant::now();
    let mut step_time = Instant::now();

    println!("Generating elevation with seed {}...", seed);
    let elevation = generate_elevation(seed);
    println!("Elevation took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Classifying terrain...");
    let terrain = terrain::classify_terrain(&elevation);
    println!("Terrain classification took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Labeling regions...");
    let region_map = terrain::label_regions(&terrain);
    println!("Region labeling took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Finding city slots...");
    let city_slots = find_city_slots(&terrain);
    println!("City slots finding took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Filtering city slots by region...");
    let filtered_city_slots = filter_city_slots_by_region(&city_slots, &region_map, MIN_CITY_SLOTS_PER_ISLAND);
    println!("City slots filtering took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("City slots after filtering: {}", filtered_city_slots.len());
    if filtered_city_slots.is_empty() {
        println!("No valid city slots found, exiting.");
        return;
    }
    println!("City slots filtering took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Generating image chunks...");
    generate_image_chunks(&terrain, &elevation, &filtered_city_slots);
    println!("Image generation took {:.2?}", step_time.elapsed());

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
