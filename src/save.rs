//! World file I/O -- chunked binary format.
//!
//! ## Format version 1
//!
//! Header layout (in order):
//!   magic(4) · version(1) · config · width(2) · height(2) · chunk_size(2)
//!   · chunks_x(2) · chunks_y(2) · num_cities(4) · city_slots · city_resources
//!   · num_villages(4) · villages · chunk_index · chunk_data
//!
//! Villages were added to version 1 before any production worlds were generated,
//! so there is no legacy v1 format without villages.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::biome::CityResources;
use crate::config::WorldConfig;
use crate::terrain::Terrain;
use crate::village::{Village, VillageTrade, TradeResource};

// ---------------------------------------------------------------------------
// Magic & version
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 4] = b"WGCH";
const FORMAT_VERSION: u8 = 1;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Everything produced by the generation pipeline, ready to be serialized.
pub struct WorldData {
    pub config: WorldConfig,
    pub width: u32,
    pub height: u32,
    /// Row-major elevation grid (f32, sufficient for rendering).
    pub elevation: Vec<f32>,
    /// Row-major terrain type (`Terrain::to_u8()`).
    pub terrain: Vec<u8>,
    /// Row-major region labels (flood-fill IDs).
    pub region_labels: Vec<u32>,
    /// `(x, y)` world coordinates of every city slot.
    pub city_slots: Vec<(u32, u32)>,
    /// Row-major biome classification (`Biome::to_u8()`).
    pub biomes: Vec<u8>,
    /// Per-city aggregated resource profile, parallel to `city_slots`.
    pub city_resources: Vec<CityResources>,
    /// All villages, sorted by (region_id, y, x).
    pub villages: Vec<Village>,
}

/// A single decompressed chunk.
pub struct ChunkData {
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub elevation: Vec<f32>,
    pub region_labels: Vec<u32>,
    /// Biome classification per tile (`Biome::to_u8()`).
    pub biomes: Vec<u8>,
}

/// Metadata stored at the beginning of the chunked file.
pub struct ChunkedWorldHeader {
    pub config: WorldConfig,
    pub width: u32,
    pub height: u32,
    pub chunks_x: u32,
    pub chunks_y: u32,
    pub city_slots: Vec<(u32, u32)>,
    /// File format version.
    pub format_version: u8,
    /// Per-city aggregated resource profiles, parallel to `city_slots`.
    pub city_resources: Vec<CityResources>,
    /// All villages stored in the world file. Always populated for version-1 files.
    pub villages: Vec<Village>,
}

/// Random-access reader for the chunked world file.
pub struct ChunkedWorldReader {
    pub header: ChunkedWorldHeader,
    index: Vec<ChunkIndexEntry>,
    path: String,
}

struct ChunkIndexEntry {
    offset: u64,
    compressed_len: u32,
    uncompressed_len: u32,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Convert raw generation output into a flat [`WorldData`] for serialization.
pub fn build_world_data(
    elevation: Vec<Vec<f64>>,
    terrain: Vec<Vec<Terrain>>,
    region_labels: Vec<Vec<usize>>,
    city_slots: &[(usize, usize)],
    biomes: Vec<Vec<u8>>,
    city_resources: Vec<CityResources>,
    villages: Vec<Village>,
    config: WorldConfig,
) -> WorldData {
    let height = elevation.len() as u32;
    let width  = elevation[0].len() as u32;

    let flat_elevation: Vec<f32> = elevation.iter().flatten().map(|&e| e as f32).collect();
    let flat_terrain: Vec<u8>    = terrain.iter().flatten().map(|t| t.to_u8()).collect();
    let flat_regions: Vec<u32>   = region_labels.iter().flatten().map(|&r| r as u32).collect();
    let flat_biomes: Vec<u8>     = biomes.into_iter().flatten().collect();
    let city_slots = city_slots.iter().map(|&(x, y)| (x as u32, y as u32)).collect();

    WorldData {
        config,
        width,
        height,
        elevation: flat_elevation,
        terrain: flat_terrain,
        region_labels: flat_regions,
        city_slots,
        biomes: flat_biomes,
        city_resources,
        villages,
    }
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Serialize a [`WorldData`] into the chunked binary format.
pub fn save_world_chunked(path: &str, data: &WorldData) -> io::Result<()> {
    let width      = data.width;
    let height     = data.height;
    let chunk_size = data.config.chunk_size as u32;
    let chunks_x   = (width  + chunk_size - 1) / chunk_size;
    let chunks_y   = (height + chunk_size - 1) / chunk_size;
    let num_chunks = (chunks_x * chunks_y) as usize;

    let mut f = BufWriter::new(File::create(path)?);

    // -- Header -------------------------------------------------------------
    f.write_all(MAGIC)?;
    write_u8(&mut f, FORMAT_VERSION)?;
    write_config(&mut f, &data.config)?;
    write_u16(&mut f, width as u16)?;
    write_u16(&mut f, height as u16)?;
    write_u16(&mut f, data.config.chunk_size)?;
    write_u16(&mut f, chunks_x as u16)?;
    write_u16(&mut f, chunks_y as u16)?;

    // Cities
    write_u32(&mut f, data.city_slots.len() as u32)?;
    for &(x, y) in &data.city_slots {
        write_u16(&mut f, x as u16)?;
        write_u16(&mut f, y as u16)?;
    }

    // Per-city resource profiles
    for cr in &data.city_resources {
        write_i16(&mut f, cr.wood)?;
        write_i16(&mut f, cr.stone)?;
        write_i16(&mut f, cr.food)?;
        write_i16(&mut f, cr.metal)?;
        write_i16(&mut f, cr.favor)?;
        write_u8(&mut f, cr.gold_nodes)?;
        write_u8(&mut f, cr.dominant_biome)?;
    }

    // Villages
    // Layout per village: x(2) y(2) region_id(4) biome(1) offers(1) demands(1) = 11 bytes
    write_u32(&mut f, data.villages.len() as u32)?;
    for v in &data.villages {
        write_u16(&mut f, v.x)?;
        write_u16(&mut f, v.y)?;
        write_u32(&mut f, v.region_id)?;
        write_u8(&mut f, v.biome)?;
        write_u8(&mut f, v.trade.offers.to_u8())?;
        write_u8(&mut f, v.trade.demands.to_u8())?;
    }

    // -- Chunk index (placeholder, back-patched later) ----------------------
    let index_offset = f.stream_position()?;
    for _ in 0..num_chunks {
        f.write_all(&[0u8; 16])?;
    }

    // -- Chunk data ---------------------------------------------------------
    let mut entries: Vec<(u64, u32, u32)> = Vec::with_capacity(num_chunks);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let cw = chunk_size.min(width  - cx * chunk_size);
            let ch = chunk_size.min(height - cy * chunk_size);
            let pixels = (cw * ch) as usize;

            let mut raw = Vec::with_capacity(pixels * 6);
            for ly in 0..ch {
                for lx in 0..cw {
                    let gx  = (cx * chunk_size + lx) as usize;
                    let gy  = (cy * chunk_size + ly) as usize;
                    let idx = gy * width as usize + gx;
                    raw.push(data.terrain[idx]);
                    let elev_u16 = (data.elevation[idx].clamp(0.0, 1.0) * 65535.0) as u16;
                    raw.extend_from_slice(&elev_u16.to_le_bytes());
                    raw.extend_from_slice(&(data.region_labels[idx] as u16).to_le_bytes());
                    raw.push(data.biomes[idx]);
                }
            }

            let uncompressed_len = raw.len() as u32;
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&raw)?;
            let compressed     = encoder.finish()?;
            let compressed_len = compressed.len() as u32;
            let offset         = f.stream_position()?;
            f.write_all(&compressed)?;
            entries.push((offset, compressed_len, uncompressed_len));
        }
    }

    // -- Back-patch chunk index ---------------------------------------------
    f.seek(SeekFrom::Start(index_offset))?;
    for &(offset, compressed_len, uncompressed_len) in &entries {
        write_u64(&mut f, offset)?;
        write_u32(&mut f, compressed_len)?;
        write_u32(&mut f, uncompressed_len)?;
    }

    f.flush()?;
    eprintln!("Saved chunked world to {path}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Read the seed from an existing world file without loading the full index.
///
/// Returns `None` if the file is missing or invalid.
pub fn read_seed_from_file(path: &str) -> Option<u32> {
    let mut f = BufReader::new(File::open(path).ok()?);

    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).ok()?;
    if &magic != MAGIC { return None; }

    let version = read_u8(&mut f).ok()?;
    if version == 0 || version > FORMAT_VERSION { return None; }

    let config = read_config(&mut f).ok()?;
    Some(config.seed)
}

impl ChunkedWorldReader {
    /// Open a chunked world file and read its header + index.
    pub fn open(path: &str) -> io::Result<Self> {
        let mut f = BufReader::new(File::open(path)?);

        // Magic
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not a WGCH file"));
        }

        // Version
        let version = read_u8(&mut f)?;
        if version == 0 || version > FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported format version {version}"),
            ));
        }

        // Config
        let mut config = read_config(&mut f)?;
        let width      = read_u16(&mut f)? as u32;
        let height     = read_u16(&mut f)? as u32;
        let chunk_size = read_u16(&mut f)?;
        config.chunk_size = chunk_size;
        let chunks_x = read_u16(&mut f)? as u32;
        let chunks_y = read_u16(&mut f)? as u32;

        // Cities
        let num_cities = read_u32(&mut f)?;
        let mut city_slots = Vec::with_capacity(num_cities as usize);
        for _ in 0..num_cities {
            let x = read_u16(&mut f)? as u32;
            let y = read_u16(&mut f)? as u32;
            city_slots.push((x, y));
        }

        // Per-city resource profiles
        let mut city_resources = Vec::with_capacity(num_cities as usize);
        for _ in 0..num_cities {
            city_resources.push(CityResources {
                wood:           read_i16(&mut f)?,
                stone:          read_i16(&mut f)?,
                food:           read_i16(&mut f)?,
                metal:          read_i16(&mut f)?,
                favor:          read_i16(&mut f)?,
                gold_nodes:     read_u8(&mut f)?,
                dominant_biome: read_u8(&mut f)?,
            });
        }

        // Villages — always present in format version 1.
        let num_villages = read_u32(&mut f)?;
        let mut villages = Vec::with_capacity(num_villages as usize);
        for _ in 0..num_villages {
            villages.push(Village {
                x:         read_u16(&mut f)?,
                y:         read_u16(&mut f)?,
                region_id: read_u32(&mut f)?,
                biome:     read_u8(&mut f)?,
                trade: VillageTrade {
                    offers:  TradeResource::from_u8(read_u8(&mut f)?),
                    demands: TradeResource::from_u8(read_u8(&mut f)?),
                },
            });
        }

        // Chunk index
        let num_chunks = (chunks_x * chunks_y) as usize;
        let mut index  = Vec::with_capacity(num_chunks);
        for _ in 0..num_chunks {
            index.push(ChunkIndexEntry {
                offset:           read_u64(&mut f)?,
                compressed_len:   read_u32(&mut f)?,
                uncompressed_len: read_u32(&mut f)?,
            });
        }

        let header = ChunkedWorldHeader {
            config,
            width,
            height,
            chunks_x,
            chunks_y,
            city_slots,
            format_version: version,
            city_resources,
            villages,
        };

        Ok(Self { header, index, path: path.to_owned() })
    }

    /// Decompress and return the chunk at `(cx, cy)`.
    pub fn load_chunk(&self, cx: u32, cy: u32) -> io::Result<ChunkData> {
        let h          = &self.header;
        let chunk_size = h.config.chunk_size as u32;
        if cx >= h.chunks_x || cy >= h.chunks_y {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid chunk ({cx},{cy}) for grid {}x{}", h.chunks_x, h.chunks_y),
            ));
        }
        let idx   = (cy * h.chunks_x + cx) as usize;
        let entry = &self.index[idx];

        let mut f = BufReader::new(File::open(&self.path)?);
        f.seek(SeekFrom::Start(entry.offset))?;

        let mut compressed = vec![0u8; entry.compressed_len as usize];
        f.read_exact(&mut compressed)?;

        let mut raw = vec![0u8; entry.uncompressed_len as usize];
        DeflateDecoder::new(&compressed[..]).read_exact(&mut raw)?;

        let cw = chunk_size.min(h.width  - cx * chunk_size);
        let ch = chunk_size.min(h.height - cy * chunk_size);
        let pixels = (cw * ch) as usize;

        let mut terrain      = Vec::with_capacity(pixels);
        let mut elevation    = Vec::with_capacity(pixels);
        let mut region_labels = Vec::with_capacity(pixels);
        let mut biomes       = Vec::with_capacity(pixels);

        let mut cursor = Cursor::new(&raw);
        for _ in 0..pixels {
            let mut t = [0u8; 1];
            cursor.read_exact(&mut t)?;
            terrain.push(t[0]);

            let mut e = [0u8; 2];
            cursor.read_exact(&mut e)?;
            elevation.push(u16::from_le_bytes(e) as f32 / 65535.0);

            let mut r = [0u8; 2];
            cursor.read_exact(&mut r)?;
            region_labels.push(u16::from_le_bytes(r) as u32);

            let mut b = [0u8; 1];
            cursor.read_exact(&mut b)?;
            biomes.push(b[0]);
        }

        Ok(ChunkData { width: cw, height: ch, terrain, elevation, region_labels, biomes })
    }
}

// ---------------------------------------------------------------------------
// Config serialization
// ---------------------------------------------------------------------------

/// Write all generation parameters. Types match the wire format directly.
fn write_config(w: &mut impl Write, c: &WorldConfig) -> io::Result<()> {
    write_u16(w, c.map_size)?;
    write_f32(w, c.scale)?;
    write_u8(w, c.octaves)?;
    write_f32(w, c.persistence)?;
    write_f32(w, c.lacunarity)?;
    write_u32(w, c.seed)?;
    write_f32(w, c.water_threshold)?;
    write_u8(w, c.city_spacing)?;
    write_u8(w, c.min_city_slots_per_island)?;
    write_u16(w, c.playable_radius)?;
    write_u16(w, c.farland_margin)?;
    write_u16(w, c.min_water_body_size)?;
    write_u8(w, c.min_land_neighbors)?;
    write_u8(w, c.min_water_neighbors)?;
    Ok(())
}

/// Read all generation parameters. Types match the wire format directly.
fn read_config(r: &mut impl Read) -> io::Result<WorldConfig> {
    let map_size              = read_u16(r)?;
    let scale                 = read_f32(r)?;
    let octaves               = read_u8(r)?;
    let persistence           = read_f32(r)?;
    let lacunarity            = read_f32(r)?;
    let seed                  = read_u32(r)?;
    let water_threshold       = read_f32(r)?;
    let city_spacing          = read_u8(r)?;
    let min_city_slots_per_island = read_u8(r)?;
    let playable_radius       = read_u16(r)?;
    let farland_margin        = read_u16(r)?;
    let min_water_body_size   = read_u16(r)?;
    let min_land_neighbors    = read_u8(r)?;
    let min_water_neighbors   = read_u8(r)?;

    Ok(WorldConfig {
        map_size,
        chunk_size: 0, // filled by the header reader
        seed,
        scale,
        octaves,
        persistence,
        lacunarity,
        water_threshold,
        playable_radius,
        farland_margin,
        city_spacing,
        min_city_slots_per_island,
        min_water_body_size,
        min_land_neighbors,
        min_water_neighbors,
        // Village params not stored in binary (tunable at generation time only)
        village_alpha: 1.2,
        village_beta:  0.60,
        village_min_ocean_distance: 12,
        village_spacing: 30,
    })
}

// ---------------------------------------------------------------------------
// Binary I/O helpers
// ---------------------------------------------------------------------------

fn write_u8(w: &mut impl Write, v: u8)   -> io::Result<()> { w.write_all(&[v]) }
fn write_u16(w: &mut impl Write, v: u16) -> io::Result<()> { w.write_all(&v.to_le_bytes()) }
fn write_i16(w: &mut impl Write, v: i16) -> io::Result<()> { w.write_all(&v.to_le_bytes()) }
fn write_u32(w: &mut impl Write, v: u32) -> io::Result<()> { w.write_all(&v.to_le_bytes()) }
fn write_u64(w: &mut impl Write, v: u64) -> io::Result<()> { w.write_all(&v.to_le_bytes()) }
fn write_f32(w: &mut impl Write, v: f32) -> io::Result<()> { w.write_all(&v.to_le_bytes()) }

fn read_u8(r: &mut impl Read)  -> io::Result<u8>  { let mut b = [0u8;1]; r.read_exact(&mut b)?; Ok(b[0]) }
fn read_u16(r: &mut impl Read) -> io::Result<u16> { let mut b = [0u8;2]; r.read_exact(&mut b)?; Ok(u16::from_le_bytes(b)) }
fn read_i16(r: &mut impl Read) -> io::Result<i16> { let mut b = [0u8;2]; r.read_exact(&mut b)?; Ok(i16::from_le_bytes(b)) }
fn read_u32(r: &mut impl Read) -> io::Result<u32> { let mut b = [0u8;4]; r.read_exact(&mut b)?; Ok(u32::from_le_bytes(b)) }
fn read_u64(r: &mut impl Read) -> io::Result<u64> { let mut b = [0u8;8]; r.read_exact(&mut b)?; Ok(u64::from_le_bytes(b)) }
fn read_f32(r: &mut impl Read) -> io::Result<f32> { let mut b = [0u8;4]; r.read_exact(&mut b)?; Ok(f32::from_le_bytes(b)) }
