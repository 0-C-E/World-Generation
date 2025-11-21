use crate::terrain::{Terrain, is_large_water_region};
use crate::{CITY_RADIUS, CITY_SPACING, MAP_SIZE};
use std::collections::HashMap;

pub fn find_city_slots(terrain: &Vec<Vec<Terrain>>) -> Vec<(usize, usize)> {
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
    let mut water_count = 0;
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
                    Terrain::Water => {
                        water_count += 1;
                        water_neighbors.push((nx, ny));
                    }
                    Terrain::FarLand => {}
                }
            }
        }
    }
    (land_count, water_count, water_neighbors)
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

pub fn filter_city_slots_by_region(city_slots: &Vec<(usize, usize)>, region_map: &Vec<Vec<usize>>, min_slots: usize) -> Vec<(usize, usize)> {
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
