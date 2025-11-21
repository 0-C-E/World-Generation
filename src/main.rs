use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::sync::Arc;
use std::time::Instant;

use image::{RgbImage, Rgb};
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;

// Constants
const MAP_SIZE: usize = 10_000;
const SCALE: f64 = 50.0;
const OCTAVES: usize = 6;
const PERSISTENCE: f64 = 0.5;
const LACUNARITY: f64 = 2.5;
const SEED: u32 = 42;

const DEEP_WATER: f64 = 0.4;
const SHALLOW_WATER: f64 = 0.55;

const CITY_SPACING: usize = 5;
const MIN_CITY_SLOTS_PER_ISLAND: usize = 6;
const CITY_RADIUS: f64 = (MAP_SIZE as f64 / 2.0) * 0.8;

const CHUNK_SIZE: usize = 250;
const CHUNK_FOLDER: &str = "chunks";

const CITY_SLOT_COLOR: Rgb<u8> = Rgb([255, 0, 0]);

// Terrain types
#[derive(Clone, Copy, PartialEq, Eq)]
enum Terrain {
    DeepWater,
    ShallowWater,
    Land,
}

fn main() {
    let start_time = Instant::now();
    let mut step_time = Instant::now();

    println!("Generating elevation...");
    let elevation = generate_elevation(SEED);
    println!("Elevation took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Classifying terrain...");
    let terrain = classify_terrain(&elevation);
    println!("Terrain classification took {:.2?}", step_time.elapsed());
    step_time = Instant::now();

    println!("Labeling regions...");
    let region_map = label_regions(&terrain);
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

/// Step 2: Classify terrain based on elevation thresholds
fn classify_terrain(elevation: &Vec<Vec<f64>>) -> Vec<Vec<Terrain>> {
    let mut terrain = vec![vec![Terrain::Land; MAP_SIZE]; MAP_SIZE];
    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let e = elevation[y][x];
            terrain[y][x] = if e < DEEP_WATER {
                Terrain::DeepWater
            } else if e < SHALLOW_WATER {
                Terrain::ShallowWater
            } else {
                Terrain::Land
            };
        }
    }
    terrain
}

/// Step 3: Label connected land regions (using 4-connectivity flood fill)
fn label_regions(terrain: &Vec<Vec<Terrain>>) -> Vec<Vec<usize>> {
    let mut labels = vec![vec![0; MAP_SIZE]; MAP_SIZE];
    let mut current_label = 1;

    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            if terrain[y][x] == Terrain::Land && labels[y][x] == 0 {
                flood_fill_label(terrain, &mut labels, x, y, current_label);
                current_label += 1;
            }
        }
    }

    labels
}

fn flood_fill_label(terrain: &Vec<Vec<Terrain>>, labels: &mut Vec<Vec<usize>>, start_x: usize, start_y: usize, label: usize) {
    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));
    labels[start_y][start_x] = label;

    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in neighbors_4(x, y) {
            if nx < MAP_SIZE && ny < MAP_SIZE {
                if terrain[ny][nx] == Terrain::Land && labels[ny][nx] == 0 {
                    labels[ny][nx] = label;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}

/// Get 4-directional neighbors
fn neighbors_4(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut n = Vec::new();
    if x > 0 {
        n.push((x - 1, y));
    }
    if x + 1 < MAP_SIZE {
        n.push((x + 1, y));
    }
    if y > 0 {
        n.push((x, y - 1));
    }
    if y + 1 < MAP_SIZE {
        n.push((x, y + 1));
    }
    n
}

/// Step 4: Check if water region is large enough (>= min_size) via flood fill
fn is_large_water_region(terrain: &Vec<Vec<Terrain>>, x: usize, y: usize, min_size: usize) -> bool {
    let terrain_type = terrain[y][x];
    if terrain_type != Terrain::DeepWater && terrain_type != Terrain::ShallowWater {
        return false;
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((x, y));

    while let Some((cx, cy)) = queue.pop_front() {
        if visited.len() >= min_size {
            break;
        }
        if !visited.insert((cx, cy)) {
            continue;
        }
        for (nx, ny) in neighbors_4(cx, cy) {
            if nx < MAP_SIZE && ny < MAP_SIZE {
                if terrain[ny][nx] == terrain_type && !visited.contains(&(nx, ny)) {
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    visited.len() >= min_size
}

/// Step 5: Find city slots based on terrain constraints
fn find_city_slots(terrain: &Vec<Vec<Terrain>>) -> Vec<(usize, usize)> {
    let mut slots = Vec::new();
    let center = MAP_SIZE / 2;
    let mut taken = vec![vec![false; MAP_SIZE]; MAP_SIZE];

    for y in CITY_SPACING..(MAP_SIZE - CITY_SPACING) {
        for x in CITY_SPACING..(MAP_SIZE - CITY_SPACING) {
            let dx = (x as isize - center as isize) as f64;
            let dy = (y as isize - center as isize) as f64;
            let dist_to_center = (dx * dx + dy * dy).sqrt();

            if dist_to_center > CITY_RADIUS {
                continue;
            }
            if terrain[y][x] == Terrain::Land {
                let (land_neighbors, shallow_neighbors, water_neighbors) = count_direct_neighbors(terrain, x, y);

                if land_neighbors >= 2 && shallow_neighbors >= 2 {
                    if !area_taken(&taken, x, y, CITY_SPACING) {
                        if water_neighbors.iter().any(|&(wx, wy)| is_large_water_region(terrain, wx, wy, 500)) {
                            slots.push((x, y));
                            mark_area_taken(&mut taken, x, y, CITY_SPACING);
                        }
                    }
                }
            }
        }
    }
    slots
}

fn count_direct_neighbors(terrain: &Vec<Vec<Terrain>>, x: usize, y: usize) -> (usize, usize, Vec<(usize, usize)>) {
    let mut land_count = 0;
    let mut shallow_water_count = 0;
    let mut water_neighbors = Vec::new();

    for &(dx, dy) in &[(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
        let nx_isize = x as isize + dx;
        let ny_isize = y as isize + dy;
        if nx_isize >= 0 && ny_isize >= 0 {
            let nx = nx_isize as usize;
            let ny = ny_isize as usize;
            if nx < MAP_SIZE && ny < MAP_SIZE {
                match terrain[ny][nx] {
                    Terrain::Land => land_count += 1,
                    Terrain::ShallowWater => {
                        shallow_water_count += 1;
                        water_neighbors.push((nx, ny));
                    }
                    Terrain::DeepWater => water_neighbors.push((nx, ny)),
                }
            }
        }
    }
    (land_count, shallow_water_count, water_neighbors)
}

fn area_taken(taken: &Vec<Vec<bool>>, x: usize, y: usize, spacing: usize) -> bool {
    let y_start = y.saturating_sub(spacing);
    let y_end = (y + spacing + 1).min(MAP_SIZE);
    let x_start = x.saturating_sub(spacing);
    let x_end = (x + spacing + 1).min(MAP_SIZE);

    for yy in y_start..y_end {
        for xx in x_start..x_end {
            if taken[yy][xx] {
                return true;
            }
        }
    }
    false
}

fn mark_area_taken(taken: &mut Vec<Vec<bool>>, x: usize, y: usize, spacing: usize) {
    let y_start = y.saturating_sub(spacing);
    let y_end = (y + spacing + 1).min(MAP_SIZE);
    let x_start = x.saturating_sub(spacing);
    let x_end = (x + spacing + 1).min(MAP_SIZE);

    for yy in y_start..y_end {
        for xx in x_start..x_end {
            taken[yy][xx] = true;
        }
    }
}

/// Step 6: Filter city slots to only those in large enough regions
fn filter_city_slots_by_region(city_slots: &Vec<(usize, usize)>, region_map: &Vec<Vec<usize>>, min_slots: usize) -> Vec<(usize, usize)> {
    let mut region_slots: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();

    for &(x, y) in city_slots {
        let region_id = region_map[y][x];
        if region_id > 0 {
            region_slots.entry(region_id).or_default().push((x, y));
        }
    }

    let mut filtered = Vec::new();
    for slots in region_slots.values() {
        if slots.len() >= min_slots {
            filtered.extend(slots.iter());
        }
    }
    filtered
}

/// Step 7: Color function for terrain and elevation values
fn get_color(terrain: Terrain, elevation: f64) -> Rgb<u8> {
    match terrain {
        Terrain::DeepWater => {
            let t = ((elevation - 0.2) / (DEEP_WATER - 0.2)).clamp(0.0, 1.0);
            let r = ((1.0 - t) * 0.0 + t * 40.0) as u8;
            let g = ((1.0 - t) * (20.0 + elevation * 60.0) + t * (150.0 + elevation * 60.0)) as u8;
            let b = ((1.0 - t) * (100.0 + elevation * 80.0) + t * (200.0 + elevation * 55.0)) as u8;
            Rgb([r, g, b])
        }
        Terrain::ShallowWater => {
            let t = ((elevation - DEEP_WATER) / (SHALLOW_WATER - DEEP_WATER)).clamp(0.0, 1.0);
            let r = ((1.0 - t) * (40.0 + elevation * 80.0) + t * (70.0 + elevation * 40.0)) as u8;
            let g = ((1.0 - t) * (150.0 + elevation * 60.0) + t * (150.0 + elevation * 80.0)) as u8;
            let b = ((1.0 - t) * (200.0 + elevation * 55.0) + t * (180.0 + elevation * 60.0)) as u8;
            Rgb([r, g, b])
        }
        Terrain::Land => {
            if elevation < SHALLOW_WATER + 0.02 {
                Rgb([229, 216, 176])
            } else if elevation < SHALLOW_WATER + 0.15 {
                let green_base = 120.0 + elevation * 100.0;
                let green_var = (30.0 * (elevation * 20.0).sin()) as i32;
                let r = (40.0 + elevation * 60.0) as u8;
                let g = (green_base as i32 + green_var).clamp(0, 255) as u8;
                let b = (30.0 + elevation * 40.0) as u8;
                Rgb([r, g, b])
            } else if elevation < 0.75 {
                let t = (elevation - (SHALLOW_WATER + 0.1)) / (0.7 - (SHALLOW_WATER + 0.1));
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

/// Step 8: Generate and save all image chunks with city circles, parallelized with rayon
fn generate_image_chunks(terrain: &Vec<Vec<Terrain>>, elevation: &Vec<Vec<f64>>, city_slots: &Vec<(usize, usize)>) {
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

/// Draw a filled circle of radius 3 pixels at (cx, cy) on the image
fn draw_city_circle(img: &mut RgbImage, cx: usize, cy: usize, color: Rgb<u8>) {
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
