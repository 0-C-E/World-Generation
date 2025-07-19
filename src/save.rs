use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CityInfo {
    pub x: usize,
    pub y: usize,
    pub region_id: usize,
}

#[derive(Serialize, Deserialize)]
pub struct IslandInfo {
    pub region_id: usize,
    pub tiles: Vec<(usize, usize)>,
    pub city_slots: Vec<CityInfo>,
    pub size: usize,
}
#[derive(Serialize, Deserialize)]
pub struct WorldSave {
    pub islands: Vec<IslandInfo>,
}
