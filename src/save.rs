//! World file I/O - chunked binary format v2.
//!
//! The file stores the world as independently-compressed chunks so that the
//! viewer can load tiles on demand. The header records all
//! [`WorldConfig`](crate::config::WorldConfig) parameters plus a chunk index
//! that maps `(cx, cy)` -> byte offset, enabling O(1) random access.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::config::WorldConfig;
use crate::terrain::Terrain;

// ---------------------------------------------------------------------------
// Magic & version
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 4] = b"WGCH";
const FORMAT_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Everything produced by the generation pipeline, ready to be serialized.
pub struct WorldData {
    pub config: WorldConfig,
    pub width: u32,
    pub height: u32,
    /// Row-major elevation grid, `f32` precision (sufficient for rendering).
    pub elevation: Vec<f32>,
    /// Row-major terrain type (`Terrain::to_u8()`).
    pub terrain: Vec<u8>,
    /// Row-major region labels (flood-fill IDs).
    pub region_labels: Vec<u32>,
    /// `(x, y)` world coordinates of every city slot.
    pub city_slots: Vec<(u32, u32)>,
}

/// A single decompressed chunk.
pub struct ChunkData {
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub elevation: Vec<f32>,
    pub region_labels: Vec<u32>,
}

/// Metadata stored at the beginning of the chunked file.
pub struct ChunkedWorldHeader {
    pub config: WorldConfig,
    pub width: u32,
    pub height: u32,
    pub chunks_x: u32,
    pub chunks_y: u32,
    pub city_slots: Vec<(u32, u32)>,
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

/// Convert raw generation output into a flat [`WorldData`] ready for
/// serialization.
pub fn build_world_data(
    elevation: Vec<Vec<f64>>,
    terrain: Vec<Vec<Terrain>>,
    region_labels: Vec<Vec<usize>>,
    city_slots: &[(usize, usize)],
    config: WorldConfig,
) -> WorldData {
    let height = elevation.len() as u32;
    let width = elevation[0].len() as u32;

    let flat_elevation: Vec<f32> = elevation.iter().flatten().map(|&e| e as f32).collect();
    let flat_terrain: Vec<u8> = terrain.iter().flatten().map(|t| t.to_u8()).collect();
    let flat_regions: Vec<u32> = region_labels
        .iter()
        .flatten()
        .map(|&r| r as u32)
        .collect();
    let city_slots = city_slots
        .iter()
        .map(|&(x, y)| (x as u32, y as u32))
        .collect();

    WorldData {
        config,
        width,
        height,
        elevation: flat_elevation,
        terrain: flat_terrain,
        region_labels: flat_regions,
        city_slots,
    }
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Serialize a [`WorldData`] into the chunked binary format.
pub fn save_world_chunked(path: &str, data: &WorldData) -> io::Result<()> {
    let width = data.width;
    let height = data.height;
    let chunk_size = data.config.chunk_size;
    let chunks_x = (width + chunk_size - 1) / chunk_size;
    let chunks_y = (height + chunk_size - 1) / chunk_size;
    let num_chunks = (chunks_x * chunks_y) as usize;

    let mut f = BufWriter::new(File::create(path)?);

    // -- Header -------------------------------------------------------------
    f.write_all(MAGIC)?;
    write_u32(&mut f, FORMAT_VERSION)?;
    write_config(&mut f, &data.config)?;
    write_u32(&mut f, width)?;
    write_u32(&mut f, height)?;
    write_u32(&mut f, chunk_size)?;
    write_u32(&mut f, chunks_x)?;
    write_u32(&mut f, chunks_y)?;

    // Cities
    write_u32(&mut f, data.city_slots.len() as u32)?;
    for &(x, y) in &data.city_slots {
        write_u32(&mut f, x)?;
        write_u32(&mut f, y)?;
    }

    // -- Chunk index (placeholder, back-patched later) ----------------------
    let index_offset = stream_position(&mut f)?;
    for _ in 0..num_chunks {
        f.write_all(&[0u8; 16])?; // offset(8) + compressed(4) + uncompressed(4)
    }

    // -- Chunk data ---------------------------------------------------------
    let mut entries: Vec<(u64, u32, u32)> = Vec::with_capacity(num_chunks);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let cw = chunk_size.min(width - cx * chunk_size);
            let ch = chunk_size.min(height - cy * chunk_size);
            let pixels = (cw * ch) as usize;

            let mut raw = Vec::with_capacity(pixels * 9);
            for ly in 0..ch {
                for lx in 0..cw {
                    let gx = (cx * chunk_size + lx) as usize;
                    let gy = (cy * chunk_size + ly) as usize;
                    let idx = gy * width as usize + gx;
                    raw.push(data.terrain[idx]);
                    raw.extend_from_slice(&data.elevation[idx].to_le_bytes());
                    raw.extend_from_slice(&data.region_labels[idx].to_le_bytes());
                }
            }

            let uncompressed_len = raw.len() as u32;
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&raw)?;
            let compressed = encoder.finish()?;
            let compressed_len = compressed.len() as u32;

            let offset = stream_position(&mut f)?;
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

/// Read just the seed from an existing world file without loading the full index.
///
/// Returns `None` if the file does not exist or is not a valid WGCH v2 file.
pub fn read_seed_from_file(path: &str) -> Option<u32> {
    let mut f = BufReader::new(File::open(path).ok()?);

    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }

    let version = read_u32(&mut f).ok()?;
    if version != FORMAT_VERSION {
        return None;
    }

    let config = read_config(&mut f).ok()?;
    Some(config.seed)
}

impl ChunkedWorldReader {
    /// Open an existing chunked world file and read its header + index.
    pub fn open(path: &str) -> io::Result<Self> {
        let mut f = BufReader::new(File::open(path)?);

        // Magic
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a WGCH file",
            ));
        }

        // Version
        let version = read_u32(&mut f)?;
        if version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported format version {version}"),
            ));
        }

        // Config (reads the v2 parameter block)
        let mut config = read_config(&mut f)?;
        let width = read_u32(&mut f)?;
        let height = read_u32(&mut f)?;
        let chunk_size = read_u32(&mut f)?;
        config.chunk_size = chunk_size; // stored separately in the file
        let chunks_x = read_u32(&mut f)?;
        let chunks_y = read_u32(&mut f)?;

        // Cities
        let num_cities = read_u32(&mut f)?;
        let mut city_slots = Vec::with_capacity(num_cities as usize);
        for _ in 0..num_cities {
            let x = read_u32(&mut f)?;
            let y = read_u32(&mut f)?;
            city_slots.push((x, y));
        }

        // Chunk index
        let num_chunks = (chunks_x * chunks_y) as usize;
        let mut index = Vec::with_capacity(num_chunks);
        for _ in 0..num_chunks {
            index.push(ChunkIndexEntry {
                offset: read_u64(&mut f)?,
                compressed_len: read_u32(&mut f)?,
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
        };

        Ok(Self {
            header,
            index,
            path: path.to_owned(),
        })
    }

    /// Decompress and return the chunk at `(cx, cy)`.
    pub fn load_chunk(&self, cx: u32, cy: u32) -> io::Result<ChunkData> {
        let h = &self.header;
        let chunk_size = h.config.chunk_size;
        let idx = (cy * h.chunks_x + cx) as usize;
        let entry = &self.index[idx];

        let mut f = BufReader::new(File::open(&self.path)?);
        f.seek(SeekFrom::Start(entry.offset))?;

        let mut compressed = vec![0u8; entry.compressed_len as usize];
        f.read_exact(&mut compressed)?;

        let mut raw = vec![0u8; entry.uncompressed_len as usize];
        DeflateDecoder::new(&compressed[..]).read_exact(&mut raw)?;

        let cw = chunk_size.min(h.width - cx * chunk_size);
        let ch = chunk_size.min(h.height - cy * chunk_size);
        let pixels = (cw * ch) as usize;

        let mut terrain = Vec::with_capacity(pixels);
        let mut elevation = Vec::with_capacity(pixels);
        let mut region_labels = Vec::with_capacity(pixels);

        let mut cursor = Cursor::new(&raw);
        for _ in 0..pixels {
            let mut t = [0u8; 1];
            cursor.read_exact(&mut t)?;
            terrain.push(t[0]);

            let mut e = [0u8; 4];
            cursor.read_exact(&mut e)?;
            elevation.push(f32::from_le_bytes(e));

            let mut r = [0u8; 4];
            cursor.read_exact(&mut r)?;
            region_labels.push(u32::from_le_bytes(r));
        }

        Ok(ChunkData {
            width: cw,
            height: ch,
            terrain,
            elevation,
            region_labels,
        })
    }
}

// ---------------------------------------------------------------------------
// Config serialization (backward-compatible with v2)
// ---------------------------------------------------------------------------

/// Write the generation parameters in the field order established by v2.
fn write_config(w: &mut impl Write, c: &WorldConfig) -> io::Result<()> {
    write_u32(w, c.map_size)?;
    write_f64(w, c.scale)?;
    write_u32(w, c.octaves)?;
    write_f64(w, c.persistence)?;
    write_f64(w, c.lacunarity)?;
    write_u32(w, c.seed)?;
    write_f64(w, c.water_threshold)?;
    write_u32(w, c.city_spacing)?;
    write_u32(w, c.min_city_slots_per_island)?;
    write_f64(w, c.playable_radius)?;
    write_f64(w, c.farland_margin)?;
    Ok(())
}

/// Read the v2 parameter block and fill defaults for fields added later.
fn read_config(r: &mut impl Read) -> io::Result<WorldConfig> {
    let map_size = read_u32(r)?;
    let scale = read_f64(r)?;
    let octaves = read_u32(r)?;
    let persistence = read_f64(r)?;
    let lacunarity = read_f64(r)?;
    let seed = read_u32(r)?;
    let water_threshold = read_f64(r)?;
    let city_spacing = read_u32(r)?;
    let min_city_slots_per_island = read_u32(r)?;
    let playable_radius = read_f64(r)?;
    let farland_margin = read_f64(r).unwrap_or(playable_radius * 0.1);

    Ok(WorldConfig {
        map_size,
        chunk_size: 0, // filled by the header reader (stored separately)
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
        // Fields not present in v2 - use defaults.
        min_water_body_size: 500,
        min_land_neighbors: 2,
        min_water_neighbors: 2,
    })
}

// ---------------------------------------------------------------------------
// Binary I/O helpers
// ---------------------------------------------------------------------------

fn write_u32(w: &mut impl Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}
fn write_u64(w: &mut impl Write, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}
fn write_f64(w: &mut impl Write, v: f64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(r: &mut impl Read) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_f64(r: &mut impl Read) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn stream_position(w: &mut (impl Seek + ?Sized)) -> io::Result<u64> {
    w.seek(SeekFrom::Current(0))
}
