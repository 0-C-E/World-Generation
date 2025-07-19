use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::Write;

#[derive(Serialize, Deserialize)]
pub struct CityInfo {
    pub x: usize,
    pub y: usize,
    pub region_id: usize,
}

#[derive(Serialize, Deserialize)]
pub struct IslandInfo {
    pub region_id: usize,
    pub city_slots: Vec<CityInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct WorldSave {
    pub islands: Vec<IslandInfo>,
}

pub fn save_world_data(
    region_map: &Vec<Vec<usize>>,
    filtered_city_slots: &Vec<(usize, usize)>,
    file_path: &str,
) {
    let mut islands = Vec::new();
    let mut region_tiles: std::collections::HashMap<usize, Vec<(usize, usize)>> = std::collections::HashMap::new();

    for y in 0..region_map.len() {
        for x in 0..region_map[0].len() {
            let region_id = region_map[y][x];
            if region_id > 0 {
                region_tiles.entry(region_id).or_default().push((x, y));
            }
        }
    }

    for (region_id, _) in &region_tiles {
        let city_slots: Vec<CityInfo> = filtered_city_slots.iter()
            .filter(|&&(x, y)| region_map[y][x] == *region_id)
            .map(|&(x, y)| CityInfo { x, y, region_id: *region_id })
            .collect();
        if !city_slots.is_empty() {
            islands.push(IslandInfo {
                region_id: *region_id,
                city_slots,
            });
        }
    }

    let world_save = WorldSave { islands };
    let json = serde_json::to_string_pretty(&world_save).unwrap();
    let mut file = File::create(file_path).unwrap();
    file.write_all(json.as_bytes()).unwrap();
}
