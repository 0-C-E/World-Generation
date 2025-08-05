use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::BufWriter;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct CityInfo {
    pub x: u16,
    pub y: u16,
}

#[derive(Serialize, Deserialize)]
pub struct IslandInfo {
    pub region_id: u16,
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
    let mut region_cities: HashMap<u16, Vec<CityInfo>> = HashMap::with_capacity(100);

    for &(x, y) in filtered_city_slots {
        let region_id = region_map[y][x] as u16;
        if region_id > 0 {
            region_cities.entry(region_id).or_default().push(CityInfo {
                x: x as u16,
                y: y as u16
            });
        }
    }

    let islands: Vec<IslandInfo> = region_cities.into_iter()
        .map(|(region_id, city_slots)| IslandInfo { region_id, city_slots })
        .collect();

    let world_save = WorldSave { islands };

    let file = File::create(file_path).unwrap();
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &world_save).unwrap();
}
