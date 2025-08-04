use std::time::Instant;

mod city;
mod image_gen;
mod map_gen;
mod save;
mod terrain;
mod config;

use city::{find_city_slots, filter_city_slots_by_region};
use image_gen::generate_image_chunks;
use map_gen::generate_elevation;
use save::save_world_data;
use config::MIN_CITY_SLOTS_PER_ISLAND;

// Constants
const SAVE_FILE: &str = "world_save.json";
const CHUNK_FOLDER: &str = "chunks";

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
    save_world_data(&region_map, &filtered_city_slots, SAVE_FILE);
    println!("Saved data to '{}' in {:.2?}", SAVE_FILE, step_time.elapsed());

    println!("Total generation time: {:.2?}", start_time.elapsed());
}
