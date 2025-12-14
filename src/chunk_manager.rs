use crate::spatial_index::PositionedPrimitive;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Bounding box geografica (lat/lon).
#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct GeoBBox {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

impl GeoBBox {
    pub fn normalized(self) -> Self {
        let (min_lat, max_lat) = if self.min_lat <= self.max_lat {
            (self.min_lat, self.max_lat)
        } else {
            (self.max_lat, self.min_lat)
        };
        let (min_lon, max_lon) = if self.min_lon <= self.max_lon {
            (self.min_lon, self.max_lon)
        } else {
            (self.max_lon, self.min_lon)
        };
        Self {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        }
    }

    #[inline]
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.min_lat && lat <= self.max_lat && lon >= self.min_lon && lon <= self.max_lon
    }

    #[inline]
    pub fn intersects(&self, other: &GeoBBox) -> bool {
        // Assumiamo bbox normalizzate; per sicurezza normalizziamo localmente.
        let a = (*self).clone().normalized();
        let b = other.clone().normalized();
        !(a.max_lat < b.min_lat
            || a.min_lat > b.max_lat
            || a.max_lon < b.min_lon
            || a.min_lon > b.max_lon)
    }
}

/// Config dei chunk.
///
/// Nota: usiamo una griglia su coordinate **WebMercator (EPSG:3857)** in metri,
/// così un chunk da `chunk_size_m=10_000` è ~10km x 10km.
#[derive(Debug, Clone, Copy)]
pub struct ChunkConfig {
    /// Dimensione chunk in metri (es: 10_000.0 per ~10km).
    pub chunk_size_m: f64,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size_m: 10_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ChunkId {
    x: i32,
    y: i32,
}


fn chunk_size_tag(cfg: ChunkConfig) -> i64 {
    cfg.chunk_size_m.round() as i64
}

fn chunk_file_name(id: ChunkId, cfg: ChunkConfig) -> String {
    // Un solo directory, filename include chunk size per evitare collisioni tra diverse size.
    format!("c{}_x{}_y{}.bin", chunk_size_tag(cfg), id.x, id.y)
}

// ---------------------------
// WebMercator helpers
// ---------------------------

const EARTH_RADIUS_M: f64 = 6_378_137.0;
const MAX_MERCATOR_LAT: f64 = 85.051_128_78;

#[inline]
fn clamp_lat_for_mercator(lat: f64) -> f64 {
    lat.clamp(-MAX_MERCATOR_LAT, MAX_MERCATOR_LAT)
}

#[inline]
fn mercator_x_m(lon_deg: f64) -> f64 {
    EARTH_RADIUS_M * lon_deg.to_radians()
}

#[inline]
fn mercator_y_m(lat_deg: f64) -> f64 {
    let lat = clamp_lat_for_mercator(lat_deg).to_radians();
    EARTH_RADIUS_M * (std::f64::consts::FRAC_PI_4 + lat * 0.5).tan().ln()
}

#[inline]
#[allow(dead_code)]
fn chunk_id_for_lat_lon(lat: f64, lon: f64, cfg: ChunkConfig) -> ChunkId {
    let x = mercator_x_m(lon);
    let y = mercator_y_m(lat);
    ChunkId {
        x: (x / cfg.chunk_size_m).floor() as i32,
        y: (y / cfg.chunk_size_m).floor() as i32,
    }
}

fn chunk_range_for_bbox(
    bbox: GeoBBox,
    cfg: ChunkConfig,
) -> (
    std::ops::RangeInclusive<i32>,
    std::ops::RangeInclusive<i32>,
) {
    let bbox = bbox.normalized();
    let x1 = mercator_x_m(bbox.min_lon);
    let x2 = mercator_x_m(bbox.max_lon);
    let y1 = mercator_y_m(bbox.min_lat);
    let y2 = mercator_y_m(bbox.max_lat);

    let min_x = (x1.min(x2) / cfg.chunk_size_m).floor() as i32;
    let max_x = (x1.max(x2) / cfg.chunk_size_m).floor() as i32;
    let min_y = (y1.min(y2) / cfg.chunk_size_m).floor() as i32;
    let max_y = (y1.max(y2) / cfg.chunk_size_m).floor() as i32;

    (min_x..=max_x, min_y..=max_y)
}

/// Una primitive OSM associata alla sua **bounding box geografica**.
#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct ChunkPrimitive<T> {
    pub bbox: GeoBBox,
    pub primitive: T,
}

/// Salva le primitive geolocalizzate in chunk su disco.
///
/// - Chunks “tipo Minecraft”: ogni primitive è assegnata ad un chunk in base al suo (lat,lon)
///   proiettato in WebMercator.
/// - File per chunk: `dir/c{size}_x{X}_y{Y}.bin`
pub fn save_chunks<T: bincode::Encode + Clone>(spatial: &[ChunkPrimitive<T>], dir: &str, cfg: ChunkConfig) -> std::io::Result<()> {
    let root = Path::new(dir);
    fs::create_dir_all(root)?;

    let mut buckets: HashMap<ChunkId, Vec<ChunkPrimitive<T>>> = HashMap::new();
    for p in spatial {
        let (xs, ys) = chunk_range_for_bbox(p.bbox.clone(), cfg);
        for x in xs {
            for y in ys.clone() {
                let id = ChunkId { x, y };
                buckets.entry(id).or_default().push(p.clone());
            }
        }
    }

    // Scrivi ogni chunk (nessuna esigenza di atomicità: non li leggiamo mentre li generiamo).
    buckets
        .into_iter()
        .try_for_each(|(id, prims)| -> std::io::Result<()> {


            let path = root.join(chunk_file_name(id, cfg));
            // Più veloce di tante piccole write: serializza in RAM e scrive in un colpo solo.
            let bytes =
                bincode::encode_to_vec(&prims, bincode::config::standard()).map_err(std::io::Error::other)?;
            fs::write(&path, bytes)?;
            Ok(())
        })?;

    Ok(())
}

/// Legge automaticamente tutti i chunk che intersecano `bbox` e ritorna le primitive
/// contenute dentro `bbox` (filtrate in lat/lon).
pub fn load_primitives_in_bbox<T: bincode::Decode<()> + Clone>(
    dir: &str,
    bbox: &GeoBBox,
    cfg: ChunkConfig,
) -> std::io::Result<Vec<ChunkPrimitive<T>>> {
    let root = Path::new(dir);
    let bbox = bbox.clone().normalized();
    let (xs, ys) = chunk_range_for_bbox(bbox.clone(), cfg);
    let total_to_check = xs.clone().count() * ys.clone().count();

    let mut out: Vec<ChunkPrimitive<T>> = Vec::new();
    //let mut seen: HashSet<(u8, i64)> = HashSet::new();
    let mut loaded_chunks: usize = 0;
    for x in xs {
        for y in ys.clone() {
            let id = ChunkId { x, y };
            // Formato: c{size}_x{X}_y{Y}.bin
            let new_path = root.join(chunk_file_name(id, cfg));
            let bytes = match fs::read(&new_path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            loaded_chunks += 1;

            let prims: Vec<ChunkPrimitive<T>> =
                bincode::decode_from_slice(&bytes, bincode::config::standard())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .0;

            for p in prims {
                if !p.bbox.intersects(&bbox) {
                    continue;
                }
                //let key = p.osm_key();
                //if seen.insert(key) {
                    out.push(p);
                //}
            }
        }
    }

    log::info!(
        "Loaded {loaded_chunks}/{total_to_check} chunks from '{dir}' for bbox={bbox:?} chunk_size_m={}",
        cfg.chunk_size_m
    );
    Ok(out)
}