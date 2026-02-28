# World Generator

A procedural world generator written in Rust, designed as the foundation for **[0 C.E.](https://github.com/0-C-E) -- a web-based, open-source ancient world strategy game (MMORTS)**. It creates a 10,000 x 10,000 tile ocean-and-islands map complete with terrain, biomes, cities, resource economies, and island boundaries, then lets you explore it in the browser through a zoomable map viewer.

## What is this project? (The big picture)

Imagine games like [Grepolis](https://en.wikipedia.org/wiki/Grepolis), [0 A.D.](https://play0ad.com/), [Tribal Wars](https://www.tribalwars.net/en-dk/), [Age of Empires](https://www.ageofempires.com/), or [Civilization VI](https://civilization.2k.com/) -- hundreds of players share a single world map made up of ocean, islands, and cities. Each island has a limited number of city slots that players can colonize, fight over, and develop.

**This project generates that entire game world from scratch.** It doesn't just draw a pretty picture -- it produces a structured data file that a game server could use to run an actual online strategy game. Specifically, it:

1. **Generates terrain** -- builds a realistic heightmap using [Perlin noise](https://en.wikipedia.org/wiki/Perlin_noise), then classifies every tile as water, land, or decorative border.
2. **Discovers islands** -- uses flood-fill to label connected land tiles, turning them into distinct islands with unique IDs.
3. **Places cities** -- finds valid coastal positions (next to ocean, spaced apart) and assigns them to islands, discarding tiny islands with too few slots.
4. **Classifies biomes** -- layers 6 Perlin noise fields (continentalness, elevation, erosion, temperature, peaks/valleys, divine favor) to assign each tile a strategically meaningful biome (Forest, Desert, Sacred Grove, etc.).
5. **Computes city resources** -- scans tiles around each city to aggregate production modifiers (wood, stone, food, metal, favor) and locate gold vein nodes.
6. **Saves everything** -- writes a compact binary file (`.world`) where the map is split into compressed chunks so any small area can be loaded instantly without reading the whole file.
7. **Serves a web viewer** -- runs a tiny HTTP server that renders map tiles on demand, and the browser shows a full zoomable map with island labels, city markers, and resource popups via [Leaflet.js](https://leafletjs.com/).

### Why Rust?

Rust gives us the speed to generate and serve a 100-million-tile world in under a minute, with safe memory management and no garbage collector pauses -- important for a game server that needs to stay responsive.

## How it works (step by step)

Here's the generation pipeline:

```
Perlin noise -> Elevation grid -> Terrain classification -> Region labeling
  -> Water bodies -> City placement -> Biome classification -> City resources -> Save
```

### Step 1: Elevation (heightmap)

We use **Perlin noise** -- a smooth random function that produces natural-looking hills and valleys. By layering multiple "octaves" of noise at different scales (a technique called **fractal Brownian motion / fBm**), we get large continents with fine coastal detail.

The result is a square grid of floating-point heights, normalized to `[0.0, 1.0]`.

### Step 2: Terrain classification

Each tile is assigned one of three types based on its height and distance from the map center:

| Type | Rule | Purpose |
|------|------|---------|
| **Water** | Elevation below 0.55 | Ocean and lakes |
| **Land** | Elevation >= 0.55 and within the playable radius | Colonizable terrain |
| **FarLand** | Beyond the playable radius + farland margin | Decorative border, not part of gameplay |

### Step 3: Region labeling (island detection)

A [flood-fill](https://en.wikipedia.org/wiki/Flood_fill) algorithm walks every `Land` tile and groups connected tiles into numbered regions. Each region is one island. This is the same idea as the "paint bucket" tool in image editors -- click a patch of the same color and it fills the whole connected area.

### Step 4: City placement

We scan the map for tiles that qualify as city slots:
- The tile is `Land`
- It's within the playable radius
- It has enough land neighbors **and** enough water neighbors (coastal)
- At least one adjacent water tile belongs to a large ocean body (not a tiny puddle)
- It's far enough from all previously placed cities (minimum spacing)

Then we discard islands that ended up with too few city slots (fewer than 6 by default).

### Step 5: Biome classification

Six independent Perlin noise layers produce smooth, organic biome boundaries:

| Layer | Frequency | Purpose |
|-------|-----------|---------|
| Continentalness | 0.005 | Island shape / landmass size |
| Elevation | (existing) | Height from plains to mountains |
| Erosion / Wetness | 0.015 | Fertility vs ruggedness |
| Temperature | 0.008 | Climate gradient (cold peaks to hot valleys) |
| Peaks / Valleys | 0.03 | Rare terrain features |
| Favor Harmony | 0.003 | Divine attunement zones |

Each tile is classified into one of **16 biomes** (Ocean, Coast, Beach, Plains, Forest, Swamp, Hills, Mountains, Snowy Peaks, Desert, Tundra, Valley, Highlands, Sacred Grove, Deep Harbor, Far Land). Classification uses priority rules -- rarest biomes are checked first.

### Step 6: City resources

For each city, a circular scan (radius 6 tiles, ~113 tiles) aggregates:

- **Passive modifiers** -- each biome tile contributes percentage-point bonuses/maluses to Wood, Stone, Food, Metal, and Favor.
- **Gold veins** -- thin, river-like Perlin noise contours trace gold deposits through eligible biomes (Valley, Desert, Deep Harbor, Highlands, Mountains, Snowy Peaks). Gold is never passive; each node must be actively farmed.
- **Island-size Favor multiplier** -- small islands (near the minimum player count) receive up to 3x Favor, making Sacred Grove tiles on tiny islands the strongest Favor sources in the game.
- **Dominant biome** -- the most common biome in the scan radius, shown in the city popup.

### Step 7: Saving to disk

Everything is written to a single `.world` binary file in a custom **chunked format**:
- The map is divided into chunks (256 x 256 tiles by default)
- Each chunk is independently Deflate-compressed
- A **chunk index** at the start of the file maps each chunk to its byte offset, enabling **O(1) random access** -- the viewer can jump to any chunk without decompressing the rest
- Per-city resource profiles are stored in the header for instant access

### Step 8: Web viewer

A lightweight HTTP server (`tiny_http`) reads the `.world` file and serves:
- **Map tiles** -- rendered as 256 x 256 PNG images on demand, colored by biome
- **City data** -- JSON array of all city positions with resource profiles
- **Island data** -- JSON array of island summaries (centroid, city count, bounding box)
- **Island outlines** -- boundary polylines for display on the map
- **Debug tiles** -- diagnostic overlays with tile grid borders, coordinates, and gold vein visualization

The browser frontend uses Leaflet.js (a popular interactive map library) to display the tiles in a Google Maps-like zoomable interface. A spatial grid index and viewport culling keep rendering fast even with 100k+ cities.

## The `.world` binary file format

The file uses a custom binary format (magic bytes: `WGCH`). All values are **little-endian**.

```
+---------------------------------------------+
|  Header                                     |
|  +- Magic: "WGCH" (4 bytes)                |
|  +- Version: 1 (u8)                        |
|  +- Config block (generation parameters)    |
|  +- Width, Height, ChunkSize (u16 each)     |
|  +- ChunksX, ChunksY (u16 each)             |
|  +- NumCities (u32)                         |
|  +- City slots: [(x: u16, y: u16); N]       |
|  +- City resources:                         |
|     [(wood, stone, food, metal, favor): i16, |
|      gold_nodes: u8, dominant_biome: u8; N]  |
+---------------------------------------------+
|  Chunk Index (one entry per chunk)          |
|  +- [offset: u64, comp_len: u32,            |
|      uncomp_len: u32] x (ChunksX*ChunksY)   |
+---------------------------------------------+
|  Chunk Data (Deflate-compressed blocks)     |
|  Per tile (6 bytes):                        |
|    terrain (u8) + elevation (u16)           |
|    + region_label (u16) + biome (u8)        |
+---------------------------------------------+
```

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

Produces a `world.world` file containing the full map data. If the file already exists with the same seed, generation is skipped.

### 2. Start the web viewer

```bash
cargo run --release --bin viewer
```

To load a different file:

```bash
cargo run --release --bin viewer -- path/to/other.world
```

Open **http://localhost:8080** in your browser to explore the generated world. The viewer pre-computes island and city data at startup, then serves tiles on demand.

A **debug viewer** is available at **http://localhost:8080/debug** with tile grid borders, coordinate overlays, and gold vein visualization.

## Configuration

All parameters are read from **environment variables**, with `.env` file support via `dotenvy`. No recompilation needed -- just edit `.env` and restart.

Example `.env`:

```env
MAP_SIZE=10000
SEED=511652490
WATER_THRESHOLD=0.55
CITY_SPACING=5
```

| Parameter | Default | Env var | Description |
|-----------|---------|---------|-------------|
| `map_size` | 10,000 | `MAP_SIZE` | Side length of the square world (tiles) |
| `chunk_size` | auto | `CHUNK_SIZE` | Side length of one chunk (`auto` picks optimal) |
| `seed` | random | `SEED` | Perlin noise seed (deterministic generation) |
| `scale` | 50.0 | `SCALE` | Base noise frequency (higher = more detail) |
| `octaves` | 6 | `OCTAVES` | Fractal noise layers |
| `persistence` | 0.5 | `PERSISTENCE` | Amplitude decay per octave |
| `lacunarity` | 2.5 | `LACUNARITY` | Frequency multiplier per octave |
| `water_threshold` | 0.55 | `WATER_THRESHOLD` | Elevation below this = water |
| `farland_margin` | 2 x city_spacing | `FARLAND_MARGIN` | Gap (tiles) between playable area and FarLand |
| `city_spacing` | 5 | `CITY_SPACING` | Minimum tile gap between cities |
| `min_city_slots_per_island` | 6 | `MIN_CITY_SLOTS_PER_ISLAND` | Islands with fewer slots are discarded |
| `min_water_body_size` | 500 | `MIN_WATER_BODY_SIZE` | Minimum ocean size (tiles) for coastal check |
| `min_land_neighbors` | 2 | `MIN_LAND_NEIGHBORS` | Land neighbors required for a city slot |
| `min_water_neighbors` | 2 | `MIN_WATER_NEIGHBORS` | Water neighbors required for a city slot |

The viewer also supports:

| Env var | Default | Description |
|---------|---------|-------------|
| `HOST` | `0.0.0.0:8080` | Address the HTTP server binds to |

## Project structure

```
src/
  biome.rs         Biome classification, resource modifiers, gold veins, city resources
  city.rs          Coastal city slot placement and filtering
  color.rs         Biome/elevation to RGB color mapping
  config.rs        WorldConfig -- every tunable parameter in one place
  elevation.rs     Perlin noise heightmap generation
  font.rs          Minimal 5x7 bitmap font for debug overlays
  island.rs        Island discovery (bounding boxes, centroids, city counts)
  lib.rs           Module declarations and re-exports
  main.rs          Generation CLI entry point
  save.rs          Chunked binary .world format (writer + reader)
  terrain.rs       Terrain classification + flood-fill region labeling
  tile.rs          256x256 PNG tile renderer (normal + debug modes)
  world.rs         World facade -- wraps the file reader with a chunk cache
  bin/
    viewer.rs      HTTP tile server (tiles, cities, islands, outlines, debug)

static/
  index.html       Leaflet.js map viewer frontend
  style.css        Viewer styles (island icons, city popups, resource colors)
  viewer.js        Map interaction, spatial index, city/island layer management
  debug.html       Debug viewer frontend
  debug.css        Debug panel styles
  debug.js         Debug panel with tile grid info and coordinate tracking
  city-icon.svg    City marker icon
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `noise` | Perlin noise generation |
| `rand` | Random number generation / seeding |
| `rayon` | Parallel iteration (used during generation) |
| `flate2` | Deflate compression for `.world` chunk data |
| `png` | PNG encoding for map tiles |
| `tiny_http` | Lightweight HTTP server for the viewer |
| `dotenvy` | Load `.env` files for configuration |

## DevContainer

The project includes a DevContainer configuration (`.devcontainer/`) for VS Code:

- **Base image**: `rust:alpine3.22`
- **Extensions**: rust-analyzer, CodeLLDB, Even Better TOML, Dependi
- **Post-create**: runs `cargo build` automatically
