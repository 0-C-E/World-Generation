//! World generation CLI - procedural map generation for 0 C.E.
//!
//! Generates a complete procedural world from a single random seed,
//! including terrain, islands, cities, biomes, resources, and villages,
//! saving everything to a chunked binary file for efficient access.
//!
//! # Usage
//! ```text
//! cargo run --release
//! ```
//!
//! Configuration is read from environment variables and `.env` file (if present).
//! If a world file with the same seed already exists, generation is skipped.
//!
//! # Environment variables
//! - `SEED`: Random seed (default: random)
//! - `MAP_SIZE`: World size in tiles (default: 10,000)
//! - `CHUNK_SIZE`: Chunk size for file storage, "auto" picks optimal (default: auto)
//! - See [`WorldConfig`] for all available parameters
//!
//! # Generation pipeline
//! 1. **Elevation**: Fractal Brownian motion (fBm) Perlin noise for heightmap
//! 2. **Terrain**: Classify tiles as Water, Land, or FarLand (decorative)
//! 3. **Region labels**: Flood-fill to discover islands and assign IDs
//! 4. **Water bodies**: Label connected water regions
//! 5. **Ocean distances**: Pre-compute distance-to-ocean for village placement
//! 6. **City slots**: Find valid coastal positions for cities
//! 7. **Biomes**: Classify terrain into 16 biome types
//! 8. **Resources**: Compute production modifiers and gold vein locations
//! 9. **Villages**: Place inland villages with trade specialization
//! 10. **Save**: Write everything to chunked, compressed binary format

use std::collections::HashMap;
use std::time::Instant;

use world_generator::config::WorldConfig;
use world_generator::{biome, city, elevation, save, terrain, village};

const OUTPUT_PATH: &str = "world.world";

fn main() {
    // Load .env file if present (silently ignored if missing)
    let _ = dotenvy::dotenv();

    let config = WorldConfig::from_env();

    // Skip generation if a world file with the same seed already exists
    if let Some(existing_seed) = save::read_seed_from_file(OUTPUT_PATH) {
        if existing_seed == config.seed {
            println!(
                "Skipping generation: {} already exists with seed {}",
                OUTPUT_PATH, existing_seed
            );
            println!("To regenerate, delete the file or change SEED.");
            return;
        }
        println!(
            "Seed changed ({} → {}), regenerating world...",
            existing_seed, config.seed
        );
    }

    println!(
        "\nGenerating {}x{} world with seed={}, chunk_size={} ...\n",
        config.map_size, config.map_size, config.seed, config.chunk_size
    );

    // Phase 1: Generate heightmap using fractal Brownian motion
    let elevation_grid = timed("Elevation", || elevation::generate(&config));

    // Phase 2: Classify each tile's terrain type based on elevation and distance
    let terrain_grid = timed("Terrain", || {
        terrain::classify_terrain(
            &elevation_grid,
            config.map_len(),
            config.water_threshold,
            config.playable_radius,
            config.farland_margin,
        )
    });

    // Phase 3: Label connected land tiles as numbered regions (islands)
    let region_labels = timed("Regions", || {
        terrain::label_regions(&terrain_grid, config.map_len())
    });

    // Phase 4: Label connected water tiles as numbered water bodies
    let water_bodies = timed("Water bodies", || {
        terrain::label_water_bodies(&terrain_grid, config.map_len())
    });

    // Phase 5: Compute distance from each tile to nearest ocean/farland
    // (Used by village placement to find genuinely inland positions)
    let ocean_distances = timed("Ocean distances", || {
        terrain::compute_ocean_distances(&terrain_grid, config.map_len())
    });

    // Phase 6: Find valid coastal locations for city placement
    let city_slots = timed("City slots", || {
        city::find_city_slots(&terrain_grid, &water_bodies, &config)
    });
    println!("  Found {} candidate city slots", city_slots.len());

    // Phase 7: Filter city slots to keep only those on large enough islands
    let filtered_cities = timed("Filter islands", || {
        city::filter_city_slots_by_region(
            &city_slots,
            &region_labels,
            config.min_city_slots_per_island as usize,
        )
    });
    println!(
        "  Kept {} city slots after filtering small islands",
        filtered_cities.len()
    );

    // Phase 8: Classify terrain into 16 biome types using multiple noise layers
    let biomes = timed("Biomes", || {
        biome::generate_biomes(&config, &terrain_grid, &elevation_grid)
    });

    // Phase 9: Count cities per region (needed for Favor scaling and village placement)
    let region_city_counts: HashMap<usize, u32> = {
        let mut counts = HashMap::new();
        for &(x, y) in &filtered_cities {
            let region_id = region_labels[y][x];
            if region_id > 0 {
                *counts.entry(region_id).or_insert(0u32) += 1;
            }
        }
        counts
    };

    // Phase 10: Compute per-city resource profiles and gold vein locations
    let city_resources = timed("City resources", || {
        biome::compute_city_resources(
            &filtered_cities,
            &biomes,
            &region_labels,
            &region_city_counts,
            config.min_city_slots_per_island as u32,
            config.seed,
        )
    });
    {
        let total_gold_nodes: u32 = city_resources.iter().map(|r| r.gold_nodes as u32).sum();
        let cities_with_gold = city_resources.iter().filter(|r| r.gold_nodes > 0).count();
        println!(
            "  {}/{} cities have gold deposits ({} total nodes)",
            cities_with_gold,
            filtered_cities.len(),
            total_gold_nodes
        );
    }

    // Phase 11: Place villages on each island with trade specialization
    let villages = timed("Villages", || {
        village::place_villages(
            &terrain_grid,
            &biomes,
            &region_labels,
            &ocean_distances,
            &region_city_counts,
            &filtered_cities,
            &config,
        )
    });
    {
        // Summary of trade specializations
        let mut trade_counts = [0u32; 5];
        for village_instance in &villages {
            trade_counts[village_instance.trade.offers.to_u8() as usize] += 1;
        }
        println!("  Placed {} villages", villages.len());
    }

    // Phase 12: Package everything for binary file storage
    let world_data = timed("Build world data", || {
        save::build_world_data(
            elevation_grid,
            terrain_grid,
            region_labels,
            &filtered_cities,
            biomes,
            city_resources,
            villages,
            config.clone(),
        )
    });

    // Phase 13: Write everything to disk in chunked, compressed format
    timed("Save", || {
        match save::save_world_chunked(OUTPUT_PATH, &world_data) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: Failed to save world to {}: {}", OUTPUT_PATH, e);
                std::process::exit(1);
            }
        }
    });

    println!("\nGeneration complete. World saved to {}", OUTPUT_PATH);
}

/// Run a closure, measure its execution time, print elapsed time, and return result.
///
/// Used to track performance of each generation phase.
fn timed<T>(phase_name: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    println!("  {}: {:.2?}", phase_name, elapsed);
    result
}
