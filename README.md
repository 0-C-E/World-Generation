# World Generator

A procedural world generator written in Rust, designed as the foundation for **[0 C.E.](https://github.com/0-C-E) - a web-based, open-source ancient world strategy game (MMORTS)**. It creates a 10,000 x 10,000 tile ocean-and-islands map complete with terrain, cities, and island boundaries, then lets you explore it in the browser through a zoomable map viewer.

## What is this project? (The big picture)

Imagine games like [Grepolis](https://en.wikipedia.org/wiki/Grepolis), [0 A.D.](https://play0ad.com/), [Tribal Wars](https://www.tribalwars.net/en-dk/), [Age of Empires](https://www.ageofempires.com/), or [Civilization VI](https://civilization.2k.com/) - hundreds of players share a single world map made up of ocean, islands, and cities. Each island has a limited number of city slots that players can colonize, fight over, and develop.

**This project generates that entire game world from scratch.** It doesn't just draw a pretty picture - it produces a structured data file that a game server could use to run an actual online strategy game. Specifically, it:

1. **Generates terrain** - builds a realistic heightmap using [Perlin noise](https://en.wikipedia.org/wiki/Perlin_noise), then classifies every tile as water, land, or decorative border.
2. **Discovers islands** - uses flood-fill to label connected land tiles, turning them into distinct islands with unique IDs.
3. **Places cities** - finds valid coastal positions (next to ocean, spaced apart) and assigns them to islands, discarding tiny islands with too few slots.
4. **Saves everything** - writes a compact binary file (`.world`) where the map is split into compressed chunks so any small area can be loaded instantly without reading the whole file.
5. **Serves a web viewer** - runs a tiny HTTP server that renders map tiles on demand, and the browser shows a full zoomable map with island labels and city markers via [Leaflet.js](https://leafletjs.com/).

### Why Rust?

Rust gives us the speed to generate and serve a 100-million-tile world in under a minute, with safe memory management and no garbage collector pauses - important for a game server that needs to stay responsive.

## How it works (step by step)

Here's the pipeline that runs when you generate a world:

```
Perlin noise -> Elevation grid -> Terrain classification -> Region labeling -> City placement -> Chunked binary file
```

### Step 1: Elevation (heightmap)

We use **Perlin noise** - a smooth random function that produces natural-looking hills and valleys. By layering multiple "octaves" of noise at different scales (a technique called **fractal Brownian motion / fBm**), we get large continents with fine coastal detail.

The result is a 10,000 x 10,000 grid of floating-point heights, normalized to `[0.0, 1.0]`.

### Step 2: Terrain classification

Each tile is assigned one of three types based on its height and distance from the map center:

| Type | Rule | Purpose |
|------|------|---------|
| **Water** | Elevation below 0.55 | Ocean and lakes |
| **Land** | Elevation >= 0.55 and within the playable radius | Colonizable terrain |
| **FarLand** | Beyond the playable radius + farland margin | Decorative border, not part of gameplay |

### Step 3: Region labeling (island detection)

A [flood-fill](https://en.wikipedia.org/wiki/Flood_fill) algorithm walks every `Land` tile and groups connected tiles into numbered regions. Each region is one island. This is the same idea as the "paint bucket" tool in image editors - click a patch of the same color and it fills the whole connected area.

### Step 4: City placement

We scan the map for tiles that qualify as city slots:
- The tile is `Land`
- It's within the playable radius
- It has enough land neighbors **and** enough water neighbors (coastal)
- At least one adjacent water tile belongs to a large ocean body (not a tiny puddle)
- It's far enough from all previously placed cities (minimum spacing)

Then we discard islands that ended up with too few city slots (fewer than 6 by default).

### Step 5: Saving to disk

Everything is written to a single `.world` binary file in a custom **chunked format**:
- The map is divided into 256 x 256-tile chunks (40 x 40 = 1,600 chunks for a 10k map)
- Each chunk is independently Deflate-compressed
- A **chunk index** at the start of the file maps each chunk to its byte offset, enabling **O(1) random access** - the viewer can jump to any chunk without decompressing the rest

### Step 6: Web viewer

A lightweight HTTP server (`tiny_http`) reads the `.world` file and serves:
- **Map tiles** - rendered as 256 x 256 PNG images on demand, at zoom levels 0-8
- **City data** - JSON array of all city positions
- **Island data** - JSON array of island summaries (centroid, city count, bounding box)
- **Island outlines** - boundary polylines for display on the map

The browser frontend uses Leaflet.js (a popular interactive map library) to display the tiles in a Google Maps-like zoomable interface.

## The `.world` binary file format

The file uses a custom binary format (magic bytes: `WGCH`). All values are **little-endian**.

```
+---------------------------------------------+
|  Header                                     |
|  +- Magic: "WGCH" (4 bytes)                |
|  +- Version: 2 (u32)                       |
|  +- Config block (generation parameters)    |
|  +- Width, Height, ChunkSize (u32 each)    |
|  +- ChunksX, ChunksY (u32 each)           |
|  +- NumCities (u32)                        |
|  +- City slots: [(x: u32, y: u32); N]     |
+---------------------------------------------+
|  Chunk Index (one entry per chunk)          |
|  +- [offset: u64, comp_len: u32,           |
|      uncomp_len: u32] x (ChunksXxChunksY)  |
+---------------------------------------------+
|  Chunk Data (Deflate-compressed blocks)     |
|  +- Per pixel: terrain (u8)                |
|                elevation (f32)              |
|                region_label (u32)           |
|     = 9 bytes per pixel, compressed         |
+---------------------------------------------+
```

The config block stores: `map_size`, `scale`, `octaves`, `persistence`, `lacunarity`, `seed`, `water_threshold`, `city_spacing`, `min_city_slots_per_island`, `playable_radius`, and `farland_margin`.

## Requirements

- **Rust toolchain** (edition 2021) with Cargo

Or use the included **DevContainer** (Alpine Linux + Rust) which handles setup automatically in VS Code.

## Building

```bash
cargo build --release
```

## Usage

### 1. Generate a world

```bash
cargo run --release
```

Produces a `world.world` file (~350 MB for a 10k x 10k map) containing the full map data.

### 2. Start the web viewer

```bash
cargo run --release --bin viewer
```

To load a different file:

```bash
cargo run --release --bin viewer -- path/to/other.world
```

Open **http://localhost:8080** in your browser to explore the generated world. The viewer pre-computes island and city data at startup, then serves tiles on demand.

## Project structure

```
src/
├── config.rs       WorldConfig - every tunable parameter in one place
├── elevation.rs    Perlin noise heightmap generation
├── terrain.rs      Terrain classification + flood-fill region labeling
├── city.rs         Coastal city slot placement and filtering
├── island.rs       Island discovery (bounding boxes, centroids, city counts)
├── world.rs        World facade - wraps the file reader with a chunk cache
├── tile.rs         256x256 PNG tile renderer
├── save.rs         Chunked binary .world format (writer + reader)
├── color.rs        Elevation-to-RGB color mapping
├── lib.rs          Module declarations and re-exports
├── main.rs         Generation CLI entry point
└── bin/
    └── viewer.rs   HTTP tile server (status, tiles, cities, islands, outlines)

static/
├── index.html      Leaflet.js map viewer frontend
└── city-icon.svg   City marker icon
```

## Generation parameters

All parameters live in `WorldConfig` ([src/config.rs](src/config.rs)) and have sensible defaults:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `map_size` | 10,000 | Side length of the square world (tiles) |
| `chunk_size` | 256 | Side length of one chunk (tiles) |
| `seed` | Random positive value | Perlin noise seed |
| `scale` | 50.0 | Base noise frequency (higher = more detail) |
| `octaves` | 6 | Fractal noise layers |
| `persistence` | 0.5 | Amplitude decay per octave |
| `lacunarity` | 2.5 | Frequency multiplier per octave |
| `water_threshold` | 0.55 | Elevation below this = water |
| `farland_margin` | 2 x city_spacing | Gap (tiles) between playable area and FarLand |
| `city_spacing` | 5 | Minimum tile gap between cities |
| `min_city_slots_per_island` | 6 | Islands with fewer slots are discarded |
| `min_water_body_size` | 500 | Minimum ocean size (tiles) for coastal check |
| `min_land_neighbors` | 2 | Land neighbors required for a city slot |
| `min_water_neighbors` | 2 | Water neighbors required for a city slot |

To customize, modify the `Default` impl in [src/config.rs](src/config.rs) and regenerate.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `noise` | Perlin noise generation |
| `rand` | Random number generation |
| `rayon` | Parallel iteration (used during generation) |
| `flate2` | Deflate compression for `.world` chunk data |
| `png` | PNG encoding for map tiles |
| `tiny_http` | Lightweight HTTP server for the viewer |

## DevContainer

The project includes a DevContainer configuration (`.devcontainer/`) for VS Code:

- **Base image**: `rust:alpine3.22`
- **Extensions**: rust-analyzer, CodeLLDB, Even Better TOML, Dependi
- **Post-create**: runs `cargo build` automatically
