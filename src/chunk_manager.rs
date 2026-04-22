use crate::map_elements::MapElement;
use crate::{GeoBBox, GeoPos};
use geo::{ConvexHull, Intersects, MultiPoint};
use log::info;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub trait BlobStore {
    fn load_chunk(&self, id: ChunkId) -> std::io::Result<Vec<MapElement>>;
    fn save_chunk(&self, id: ChunkId, data: Vec<MapElement>) -> std::io::Result<()>;
}

pub struct StdFsChunkStorage {
    root: PathBuf,
}

impl StdFsChunkStorage {
    pub fn new(path: &str) -> Self {
        Self {
            root: PathBuf::from(path),
        }
    }
}

impl BlobStore for StdFsChunkStorage {
    fn load_chunk(&self, id: ChunkId) -> std::io::Result<Vec<MapElement>> {
        let new_path = self.root.join(id.file_name());
        let bytes = match fs::read(&new_path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Chunk not found",
                ));
            }
            Err(e) => return Err(e),
        };

        let prims: Vec<MapElement> =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                .0;
        Ok(prims)
    }

    fn save_chunk(&self, id: ChunkId, data: Vec<MapElement>) -> std::io::Result<()> {
        fs::create_dir_all(&self.root)?;

        let path = self.root.join(id.file_name());
        // Più veloce di tante piccole write: serializza in RAM e scrive in un colpo solo.
        let bytes = bincode::encode_to_vec(&data, bincode::config::standard())
            .map_err(std::io::Error::other)?;
        fs::write(&path, bytes)?;

        Ok(())
    }
}

pub trait GeoBBoxable {
    fn bbox(&self) -> GeoBBox;
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

impl ChunkId {
    pub fn from_pos(pos: GeoPos, cfg: ChunkConfig) -> Self {
        let (x, y) = pos.to_webmercator();
        ChunkId {
            x: (x / cfg.chunk_size_m).floor() as i32,
            y: (y / cfg.chunk_size_m).floor() as i32,
        }
    }

    pub fn bbox(&self, cfg: ChunkConfig) -> GeoBBox {
        // Chunk (x,y) copre il quadrato in metri:
        // [x*size, (x+1)*size] × [y*size, (y+1)*size] in WebMercator.
        let x0 = self.x as f64 * cfg.chunk_size_m;
        let x1 = (self.x as f64 + 1.0) * cfg.chunk_size_m;
        let y0 = self.y as f64 * cfg.chunk_size_m;
        let y1 = (self.y as f64 + 1.0) * cfg.chunk_size_m;

        let point0 = GeoPos::from_webmercator(x0, y0);
        let point1 = GeoPos::from_webmercator(x1, y1);

        GeoBBox {
            min: point0,
            max: point1,
        }
        .normalized()
    }

    pub fn file_name(&self) -> String {
        format!("c_x{}_y{}.bin", self.x, self.y)
    }
}

fn chunk_range_for_bbox(bbox: GeoBBox, cfg: ChunkConfig) -> impl IntoIterator<Item = ChunkId> {
    let bbox = bbox.normalized();

    let (x1, y1) = bbox.min.to_webmercator();
    let (x2, y2) = bbox.max.to_webmercator();

    let min_x = (x1.min(x2) / cfg.chunk_size_m).floor() as i32;
    let max_x = (x1.max(x2) / cfg.chunk_size_m).floor() as i32;
    let min_y = (y1.min(y2) / cfg.chunk_size_m).floor() as i32;
    let max_y = (y1.max(y2) / cfg.chunk_size_m).floor() as i32;

    //(min_x..=max_x, min_y..=max_y)

    (min_x..=max_x).flat_map(move |x| (min_y..=max_y).map(move |y| ChunkId { x, y }))
}

/// Salva le primitive geolocalizzate in chunk su disco.
///
/// - Chunks “tipo Minecraft”: ogni primitive è assegnata ad un chunk in base al suo (lat,lon)
///   proiettato in WebMercator.
/// - File per chunk: `dir/c{size}_x{X}_y{Y}.bin`
pub fn save_chunks(
    elements: impl IntoIterator<Item = MapElement>,
    store: &impl BlobStore,
    cfg: ChunkConfig,
) -> std::io::Result<()> {
    let mut buckets: HashMap<ChunkId, Vec<MapElement>> = HashMap::new();

    for p in elements {
        let chunks = chunk_range_for_bbox(p.bbox(), cfg);

        let multipoint = MultiPoint::new(p.to_geo().into_iter().collect());
        let hull = multipoint.convex_hull();

        for chunk in chunks {
            let chunk_bbox = chunk.bbox(cfg);

            if chunk_bbox.to_geo_rect().intersects(&hull) {
                buckets.entry(chunk).or_default().push(p.clone());
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

            store.save_chunk(id, prims)?;
            Ok(())
        })?;

    Ok(())
}

pub struct ChunkData<T: bincode::Decode<()> + Clone + GeoBBoxable> {
    pub id: ChunkId,
    pub data: Vec<T>,
    pub cfg: ChunkConfig,
}

impl<T: bincode::Decode<()> + Clone + GeoBBoxable> ChunkData<T> {
    pub fn bbox(&self) -> GeoBBox {
        self.id.bbox(self.cfg)
    }
}

/// Legge automaticamente tutti i chunk che intersecano `bbox` e ritorna le primitive
/// contenute dentro `bbox` (filtrate in lat/lon).
pub fn load_chunks_for_bbox(
    store: &impl BlobStore,
    bbox: &GeoBBox,
    cfg: ChunkConfig,
) -> std::io::Result<Vec<ChunkData<MapElement>>> {
    let bbox = bbox.clone().normalized();
    let chunks = chunk_range_for_bbox(bbox.clone(), cfg);

    let mut out: Vec<ChunkData<MapElement>> = Vec::new();
    //let mut seen: HashSet<(u8, i64)> = HashSet::new();
    for chunk in chunks {
        info!("loading chunk {:?}", chunk);
        // Formato: c{size}_x{X}_y{Y}.bin
        let prims = store.load_chunk(chunk)?;

        out.push(ChunkData {
            id: chunk,
            data: prims,
            cfg,
        });
    }

    Ok(out)
}
