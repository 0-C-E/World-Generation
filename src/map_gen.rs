use noise::{NoiseFn, Perlin};

use crate::config::{MAP_SIZE, SCALE, OCTAVES, PERSISTENCE, LACUNARITY};

pub fn generate_elevation(seed: u32) -> Vec<Vec<f64>> {
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
