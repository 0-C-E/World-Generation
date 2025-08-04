# 0 C.E. World Generator

A procedural world generation system for creating ancient Mediterranean-style worlds suitable for multiplayer strategy games. Inspired by classics like Grepolis, 0 A.D., and Civilization VI, this generator creates realistic archipelagos with strategic city placement for competitive antiquity-themed gameplay.

## 🌍 What It Does

This Rust-based world generator creates:

- **Realistic island archipelagos** using multi-octave Perlin noise
- **Strategic city slots** positioned near coastlines for naval gameplay
- **Region-based filtering** ensuring balanced island populations
- **High-resolution world maps** (5000×5000 pixels) split into manageable chunks
- **JSON world data** for game server integration

The generator is designed to run once per game server/world creation, producing both visual output for administrators and structured data for game logic.

## 🎮 Game Context

**0 C.E.** represents the ancient world setting where players:
- Found cities on predetermined strategic locations (city slots)
- Engage in naval warfare between islands
- Build civilizations in a Mediterranean-inspired environment
- Compete for territorial control in an antiquity setting

## 🏗️ How It Works

### 1. Terrain Generation
- Uses **multi-octave Perlin noise** with configurable parameters
- Generates elevation maps with realistic coastlines
- Classifies terrain into Water, Land, and FarLand (beyond playable area)

### 2. Island Detection
- **Flood-fill algorithm** identifies connected land masses
- Labels each island as a separate region
- Filters out small, non-viable landmasses

### 3. Strategic City Placement
- Finds suitable city slots on land tiles adjacent to water
- Ensures minimum spacing between cities (prevents overcrowding)
- Validates access to large water bodies (naval connectivity)
- Filters islands with insufficient city slots for balanced gameplay

### 4. Parallel Image Generation
- **Rayon-powered parallel processing** for fast chunk generation
- Splits the 5000×5000 world into 250×250 pixel chunks
- Color-codes terrain with elevation-based variations
- Marks city slots with distinctive visual indicators

### 5. Data Export
- Saves structured world data as JSON for game server integration
- Includes island information, city coordinates, and region mappings

## 🚀 Usage

### Prerequisites
- Rust (2024 edition)
- Python 3 with matplotlib and Pillow (for visualization)

### Generate a World
```bash
# Generate world data and image chunks
cargo run

# View the generated world
python3 show_chunks.py
```

### Output Files
- `chunks/` - Directory containing PNG image tiles
- `world_save.json` - Structured world data for game integration

## ⚙️ Configuration

Key parameters in `src/config.rs`:

```rust
pub const MAP_SIZE: usize = 5_000;           // World dimensions
pub const WATER: f64 = 0.55;                 // Water level threshold
pub const CITY_SPACING: usize = 5;           // Minimum distance between cities
pub const MIN_CITY_SLOTS_PER_ISLAND: usize = 6; // Island viability threshold
pub const CITY_RADIUS: f64 = (MAP_SIZE as f64 / 2.0) * 0.8; // Playable area
```

## 🖼️ Visualizing Generated Worlds

This project supports displaying the generated world using matplotlib with X11 forwarding from within the DevContainer.

### 🧭 GUI Setup by Platform

#### 🪟 Windows (with Docker Desktop)

1. **Install VcXsrv** from [sourceforge.net/projects/vcxsrv](https://sourceforge.net/projects/vcxsrv/)

2. **Launch VcXsrv with these settings**:
   - `Multiple windows`
   - `Start no client`
   - ✅ Check **Disable access control**

3. **Rebuild the DevContainer** and test:
   ```bash
   xclock  # Should open a clock window
   python3 show_chunks.py  # Display the generated world
   ```

#### 🍏 macOS (with Docker Desktop)

1. **Install XQuartz** from [xquartz.org](https://www.xquartz.org)

2. **Configure XQuartz**:
   ```bash
   defaults write org.xquartz.X11 enable_iglx -bool true
   xhost + 127.0.0.1
   ```

3. **Test the setup** as above

> **Note:** macOS X11 support can be unreliable. Consider using `plt.savefig()` as a fallback.

#### 🐧 Linux

1. **Allow local X11 access**:
   ```bash
   xhost +local:
   ```

2. **Update container configuration** to mount X11 socket and set `DISPLAY=:0`

### 🧯 Headless Alternative

If GUI display isn't available, modify `show_chunks.py`:

```python
# Replace plt.show() with:
plt.savefig("world_map.png", dpi=150, bbox_inches='tight')
print("World map saved as world_map.png")
```

## 🏎️ Performance Features

- **Parallel chunk generation** using Rayon for multi-core utilization
- **Memory-efficient processing** by generating chunks independently
- **Optimized flood-fill algorithms** for region detection
- **Configurable world sizes** from small test worlds to massive servers

## 📁 Project Structure

```
src/
├── main.rs          # Main generation pipeline
├── config.rs        # World generation parameters
├── map_gen.rs       # Perlin noise elevation generation
├── terrain.rs       # Terrain classification and region labeling
├── city.rs          # Strategic city slot placement
├── image_gen.rs     # Parallel chunk rendering
└── save.rs          # JSON world data export

show_chunks.py       # World visualization tool
```

## 🎯 Intended Audience

- **Game developers** building strategy games with procedural worlds
- **Procedural generation enthusiasts** interested in terrain algorithms
- **Indie developers** needing multiplayer-ready world generation systems

## 🔮 Future Enhancements

- Biome generation (desert, forest, mountain regions)
- Resource node placement
- Trade route optimization
- Historical name generation for cities and regions
- Integration with game server APIs

## 📝 License

[Add your license here]

---

*Generate worlds worthy of ancient empires. ⚔️*
