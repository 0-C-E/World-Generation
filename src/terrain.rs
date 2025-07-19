use std::collections::VecDeque;

use crate::config::{CITY_RADIUS, MAP_SIZE, WATER};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    Water,
    Land,
    FarLand, // Beyond world edge
}

pub fn classify_terrain(elevation: &Vec<Vec<f64>>) -> Vec<Vec<Terrain>> {
    let mut terrain = vec![vec![Terrain::Land; MAP_SIZE]; MAP_SIZE];
    let center = MAP_SIZE / 2;

    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let dx = (x as isize - center as isize) as f64;
            let dy = (y as isize - center as isize) as f64;
            let dist_to_center = (dx * dx + dy * dy).sqrt();
            let e = elevation[y][x];
            terrain[y][x] = if e < WATER {
                Terrain::Water
            } else if dist_to_center > CITY_RADIUS * 1.1 {
                Terrain::FarLand
            } else {
                Terrain::Land
            };
        }
    }
    terrain
}

pub fn label_regions(terrain: &Vec<Vec<Terrain>>) -> Vec<Vec<usize>> {
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

pub fn neighbors_4(x: usize, y: usize) -> Vec<(usize, usize)> {
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

pub fn is_large_water_region(terrain: &Vec<Vec<Terrain>>, x: usize, y: usize, min_size: usize) -> bool {
    let terrain_type = terrain[y][x];
    if terrain_type != Terrain::Water {
        return false;
    }

    let mut visited = std::collections::HashSet::new();
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
