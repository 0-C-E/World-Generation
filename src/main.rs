//! World generation CLI.
//!
//! Generates a complete world file using [`WorldConfig::default()`] parameters
//! and saves it in the chunked binary format. If a world file with the same
//! seed already exists, generation is skipped.

use std::time::Instant;

use world_generator::config::WorldConfig;
use world_generator::{city, elevation, save, terrain};

const OUTPUT_PATH: &str = "world.world";

fn main() {
    let config = WorldConfig::default();

    // Skip generation if the existing file was created with the same seed.
    if let Some(existing_seed) = save::read_seed_from_file(OUTPUT_PATH) {
        if existing_seed == config.seed {
            println!(
                "Skipping generation: {OUTPUT_PATH} already has seed {existing_seed}"
            );
            return;
        }
        println!(
            "Seed changed ({existing_seed} -> {}), regenerating...",
            config.seed
        );
    }

    println!(
        "Generating {}x{} world (seed={}, chunk_size={})...",
        config.map_size, config.map_size, config.seed, config.chunk_size
    );

    let elevation = timed("Elevation", || elevation::generate(&config));

    let terrain_grid = timed("Terrain", || {
        terrain::classify_terrain(
            &elevation,
            config.map_len(),
            config.water_threshold,
            config.playable_radius,
        )
    });

    let region_labels = timed("Regions", || {
        terrain::label_regions(&terrain_grid, config.map_len())
    });

    let city_slots = timed("City slots", || {
        city::find_city_slots(&terrain_grid, &config)
    });
    println!("  Found {} candidate city slots", city_slots.len());

    let filtered = timed("Filter islands", || {
        city::filter_city_slots_by_region(
            &city_slots,
            &region_labels,
            config.min_city_slots_per_island as usize,
        )
    });
    println!("  Kept {} city slots after island filter", filtered.len());

    let world_data = timed("Build world data", || {
        save::build_world_data(
            elevation,
            terrain_grid,
            region_labels,
            &filtered,
            config.clone(),
        )
    });

    timed("Save", || {
        save::save_world_chunked(OUTPUT_PATH, &world_data)
            .expect("Failed to save world");
    });

    println!("Done.");
}

/// Run a closure, print its wall-clock time, and return the result.
fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    println!("  {label}: {:.2?}", start.elapsed());
    result
}
