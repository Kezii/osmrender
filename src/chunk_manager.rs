use crate::GeoPos;
use crate::spatial_index::PositionedPrimitive;
use log::info;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Bounding box geografica (lat/lon).
#[derive(Default, Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct GeoBBox {
    pub min: GeoPos,
    pub max: GeoPos,
}

impl GeoBBox {
    pub fn normalized(self) -> Self {
        let min_lat = self.min.lat().min(self.max.lat());
        let max_lat = self.min.lat().max(self.max.lat());
        let min_lon = self.min.lon().min(self.max.lon());
        let max_lon = self.min.lon().max(self.max.lon());

        Self {
            min: GeoPos::new(min_lat, min_lon),
            max: GeoPos::new(max_lat, max_lon),
        }
    }

    #[inline]
    pub fn contains(&self, pos: GeoPos) -> bool {
        pos.lat() >= self.min.lat()
            && pos.lat() <= self.max.lat()
            && pos.lon() >= self.min.lon()
            && pos.lon() <= self.max.lon()
    }

    #[inline]
    pub fn intersects(&self, other: &GeoBBox) -> bool {
        // Assumiamo bbox normalizzate; per sicurezza normalizziamo localmente.
        let a = (*self).clone().normalized();
        let b = other.clone().normalized();
        !(a.max.lat() < b.min.lat()
            || a.min.lat() > b.max.lat()
            || a.max.lon() < b.min.lon()
            || a.min.lon() > b.max.lon())
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
pub struct ChunkId {
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

#[inline]
#[allow(dead_code)]
fn chunk_id_for_lat_lon(pos: GeoPos, cfg: ChunkConfig) -> ChunkId {
    let (x, y) = pos.to_webmercator();
    ChunkId {
        x: (x / cfg.chunk_size_m).floor() as i32,
        y: (y / cfg.chunk_size_m).floor() as i32,
    }
}

fn chunk_range_for_bbox(
    bbox: GeoBBox,
    cfg: ChunkConfig,
) -> (std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>) {
    let bbox = bbox.normalized();

    let (x1, y1) = bbox.min.to_webmercator();
    let (x2, y2) = bbox.max.to_webmercator();

    let min_x = (x1.min(x2) / cfg.chunk_size_m).floor() as i32;
    let max_x = (x1.max(x2) / cfg.chunk_size_m).floor() as i32;
    let min_y = (y1.min(y2) / cfg.chunk_size_m).floor() as i32;
    let max_y = (y1.max(y2) / cfg.chunk_size_m).floor() as i32;

    (min_x..=max_x, min_y..=max_y)
}

fn geo_bbox_for_chunk_id(id: ChunkId, cfg: ChunkConfig) -> GeoBBox {
    // Chunk (x,y) copre il quadrato in metri:
    // [x*size, (x+1)*size] × [y*size, (y+1)*size] in WebMercator.
    let x0 = id.x as f64 * cfg.chunk_size_m;
    let x1 = (id.x as f64 + 1.0) * cfg.chunk_size_m;
    let y0 = id.y as f64 * cfg.chunk_size_m;
    let y1 = (id.y as f64 + 1.0) * cfg.chunk_size_m;

    let point0 = GeoPos::from_webmercator(x0, y0);
    let point1 = GeoPos::from_webmercator(x1, y1);

    GeoBBox {
        min: point0,
        max: point1,
    }
    .normalized()
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
pub fn save_chunks<T: bincode::Encode + Clone + std::fmt::Debug>(
    spatial: impl IntoIterator<Item = ChunkPrimitive<T>>,
    dir: &str,
    cfg: ChunkConfig,
) -> std::io::Result<()> {
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
            info!("saving chunk {:?} with {} primitives", id, prims.len());

            if prims.len() < 50 {
                return Ok(());
            }

            let path = root.join(chunk_file_name(id, cfg));
            // Più veloce di tante piccole write: serializza in RAM e scrive in un colpo solo.
            let bytes = bincode::encode_to_vec(&prims, bincode::config::standard())
                .map_err(std::io::Error::other)?;
            fs::write(&path, bytes)?;
            Ok(())
        })?;

    Ok(())
}

pub struct ChunkData<T: bincode::Decode<()> + Clone> {
    pub id: ChunkId,
    pub data: Vec<ChunkPrimitive<T>>,
    pub cfg: ChunkConfig,
}

impl<T: bincode::Decode<()> + Clone> ChunkData<T> {
    pub fn bbox(&self) -> GeoBBox {
        geo_bbox_for_chunk_id(self.id, self.cfg)
    }
}

/// Legge automaticamente tutti i chunk che intersecano `bbox` e ritorna le primitive
/// contenute dentro `bbox` (filtrate in lat/lon).
pub fn load_chunks_for_bbox<T: bincode::Decode<()> + Clone>(
    dir: &str,
    bbox: &GeoBBox,
    cfg: ChunkConfig,
) -> std::io::Result<Vec<ChunkData<T>>> {
    let root = Path::new(dir);
    let bbox = bbox.clone().normalized();
    let (xs, ys) = chunk_range_for_bbox(bbox.clone(), cfg);

    let mut out: Vec<ChunkData<T>> = Vec::new();
    //let mut seen: HashSet<(u8, i64)> = HashSet::new();
    for x in xs {
        for y in ys.clone() {
            let id = ChunkId { x, y };
            info!("loading chunk {:?}", id);
            // Formato: c{size}_x{X}_y{Y}.bin
            let new_path = root.join(chunk_file_name(id, cfg));
            let bytes = match fs::read(&new_path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };

            let prims: Vec<ChunkPrimitive<T>> =
                bincode::decode_from_slice(&bytes, bincode::config::standard())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .0;

            out.push(ChunkData {
                id,
                data: prims,
                cfg,
            });
        }
    }

    Ok(out)
}
