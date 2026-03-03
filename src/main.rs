//! World generation CLI.
//!
//! Generates a complete world file and saves it in the chunked binary format.
//! Configuration is read from environment variables (and `.env` in dev).
//! If a world file with the same seed already exists, generation is skipped.

use std::collections::HashMap;
use std::time::Instant;

use world_generator::config::WorldConfig;
use world_generator::{biome, city, elevation, save, terrain, village};

const OUTPUT_PATH: &str = "world.world";

fn main() {
    // Load .env file if present (silently ignored if missing).
    let _ = dotenvy::dotenv();

    let config = WorldConfig::from_env();

    // Skip generation if the existing file was created with the same seed.
    if let Some(existing_seed) = save::read_seed_from_file(OUTPUT_PATH) {
        if existing_seed == config.seed {
            println!("Skipping generation: {OUTPUT_PATH} already has seed {existing_seed}");
            return;
        }
        println!("Seed changed ({existing_seed} -> {}), regenerating...", config.seed);
    }

    println!(
        "Generating {}x{} world (seed={}, chunk_size={})...",
        config.map_size, config.map_size, config.seed, config.chunk_size
    );

    let elevation_grid = timed("Elevation", || elevation::generate(&config));

    let terrain_grid = timed("Terrain", || {
        terrain::classify_terrain(
            &elevation_grid,
            config.map_len(),
            config.water_threshold,
            config.playable_radius,
            config.farland_margin,
        )
    });

    let region_labels = timed("Regions", || {
        terrain::label_regions(&terrain_grid, config.map_len())
    });

    let water_bodies = timed("Water bodies", || {
        terrain::label_water_bodies(&terrain_grid, config.map_len())
    });

    // Ocean distance map — used by village placement.
    let ocean_distances = timed("Ocean distances", || {
        terrain::compute_ocean_distances(&terrain_grid, config.map_len())
    });

    let city_slots = timed("City slots", || {
        city::find_city_slots(&terrain_grid, &water_bodies, &config)
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

    let biomes = timed("Biomes", || {
        biome::generate_biomes(&config, &terrain_grid, &elevation_grid)
    });

    // Per-region city counts (needed for Favor scaling and village counts).
    let region_city_counts: HashMap<usize, u32> = {
        let mut counts = HashMap::new();
        for &(x, y) in &filtered {
            let rid = region_labels[y][x];
            if rid > 0 {
                *counts.entry(rid).or_insert(0u32) += 1;
            }
        }
        counts
    };

    let city_resources = timed("City resources", || {
        biome::compute_city_resources(
            &filtered,
            &biomes,
            &region_labels,
            &region_city_counts,
            config.min_city_slots_per_island as u32,
            config.seed,
        )
    });
    {
        let gold_total: u32 = city_resources.iter().map(|r| r.gold_nodes as u32).sum();
        let with_gold = city_resources.iter().filter(|r| r.gold_nodes > 0).count();
        println!(
            "  {with_gold}/{} cities have gold nodes ({gold_total} total)",
            filtered.len()
        );
    }

    // Villages
    let villages = timed("Villages", || {
        village::place_villages(
            &terrain_grid,
            &biomes,
            &region_labels,
            &ocean_distances,
            &region_city_counts,
            &filtered,
            &config,
        )
    });
    {
        // Summary: total count + breakdown of offers distribution.
        let mut offer_counts = [0u32; 5];
        for v in &villages {
            offer_counts[v.trade.offers.to_u8() as usize] += 1;
        }
        println!("  Placed {} villages", villages.len());
    }

    let world_data = timed("Build world data", || {
        save::build_world_data(
            elevation_grid,
            terrain_grid,
            region_labels,
            &filtered,
            biomes,
            city_resources,
            villages,
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
    let start  = Instant::now();
    let result = f();
    println!("  {label}: {:.2?}", start.elapsed());
    result
}
