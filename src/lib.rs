#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::missing_docs
)]
#![warn(clippy::pedantic, clippy::nursery)]
//! World Generator -- procedural map generator for 0 C.E.
//!
//! # Architecture
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`biome`] | Biome classification, resource modifiers, gold veins, city resources |
//! | [`city`] | Coastal city-slot placement |
//! | [`config`] | [`WorldConfig`] -- the single source of every tunable parameter |
//! | [`elevation`] | Perlin-noise heightmap generation |
//! | [`font`] | Minimal 5x7 bitmap font for debug overlays |
//! | [`island`] | Island discovery and representation |
//! | [`save`] | Chunked, compressed binary file format |
//! | [`terrain`] | Terrain classification, region labeling, ocean distance map |
//! | [`tile`] | Slippy-map tile renderer (256 x 256 PNGs) |
//! | [`village`] | Inland village placement and trade profile computation |
//! | [`world`] | High-level [`World`] facade for game / viewer code |

pub mod biome;
pub mod city;
pub mod config;
pub mod elevation;
pub mod font;
pub mod island;
pub mod save;
pub mod terrain;
pub mod tile;
pub mod village;
pub mod world;

// Re-export key types for convenience.
pub use config::WorldConfig;
pub use world::World;
