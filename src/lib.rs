//! **World Generator** - a procedural map generator for Grepolis-style games.
//!
//! # Architecture
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`config`] | [`WorldConfig`] - the single source of every tunable parameter |
//! | [`elevation`] | Perlin-noise heightmap generation |
//! | [`terrain`] | Terrain classification and region labeling |
//! | [`city`] | Coastal city-slot placement |
//! | [`color`] | Terrain/elevation to RGB mapping for tile rendering |
//! | [`font`] | Minimal 5x7 bitmap font for debug overlays |
//! | [`save`] | Chunked, compressed binary file format (v2) |
//! | [`island`] | Island discovery and representation |
//! | [`world`] | High-level [`World`] facade for game / viewer code |
//! | [`tile`] | Slippy-map tile renderer (256 x 256 PNGs) |

pub mod config;
pub mod elevation;
pub mod terrain;
pub mod city;
pub mod color;
pub mod font;
pub mod save;
pub mod island;
pub mod world;
pub mod tile;

// Re-export key types for convenience.
pub use config::WorldConfig;
pub use world::World;
