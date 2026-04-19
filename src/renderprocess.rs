use crate::chunk_manager::{ChunkConfig, ChunkData, GeoBBox, load_chunks_for_bbox};
use crate::imageframebuffer::ImageFramebuffer;
use crate::map_elements::{ElementType, MapElement};
use crate::raw_osm_reader::{RawOsmData, RelationMemberType};
use crate::rendering_adapter::{ConversionParams, OwnedMeshData, converti_a_mesh};
use embedded_gfx::K3dengine;
use embedded_gfx::canvas::{DrawError, GFX2DCanvas};
use embedded_gfx::draw::draw;
use embedded_gfx::mesh::K3dMesh;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, Size};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use image::RgbImage;
use log::{error, info};
use nalgebra::Point3;
use std::collections::HashSet;
use std::time::Instant;

/// Switch per abilitare il rendering dei bordi dei chunk (overlay debug).
const SHOW_CHUNK_BORDERS: bool = true;
pub const MAP_SCALE_FACTOR: f32 = 0.0003;
pub const CAMERA_DISTANCE: f32 = 2.0;
pub const CAMERA_FOVY: f32 = std::f32::consts::PI / 6.0;
const CHUNK_LOAD_OVERSCAN: f64 = 1.05;

/// Calcola la distanza in metri tra due coordinate geografiche usando la formula di Haversine
fn distanza_geografica(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371000.0; // Raggio della Terra in metri

    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().asin();

    R * c
}

/// Verifica se un punto è entro un raggio specificato da un punto centrale
fn entro_raggio(lat: f64, lon: f64, centro_lat: f64, centro_lon: f64, raggio_metri: f64) -> bool {
    distanza_geografica(lat, lon, centro_lat, centro_lon) <= raggio_metri
}

fn bbox_for_viewport(
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
    viewport: Size,
) -> GeoBBox {
    let aspect_ratio = if viewport.height == 0 {
        1.0
    } else {
        viewport.width as f64 / viewport.height as f64
    };
    let aspect_ratio = aspect_ratio.max(f64::EPSILON);

    // Manteniamo `raggio_metri` come semidimensione dell'asse più corto e
    // allarghiamo l'altro asse per adattarci al viewport senza deformare la mappa.
    let (half_width_m, half_height_m) = if aspect_ratio >= 1.0 {
        (raggio_metri * aspect_ratio, raggio_metri)
    } else {
        (raggio_metri, raggio_metri / aspect_ratio)
    };

    let lat_delta = half_height_m / 111000.0;
    let meters_per_lon_degree = (111000.0 * centro_lat.to_radians().cos().abs()).max(1.0);
    let lon_delta = half_width_m / meters_per_lon_degree;

    GeoBBox {
        min_lat: centro_lat - lat_delta,
        max_lat: centro_lat + lat_delta,
        min_lon: centro_lon - lon_delta,
        max_lon: centro_lon + lon_delta,
    }
}

pub fn viewport_geo_overscan(viewport: Size) -> f64 {
    if viewport.height == 0 {
        return 1.0;
    }

    let visible_world_height = 2.0 * CAMERA_DISTANCE as f64 * (CAMERA_FOVY as f64 / 2.0).tan();
    let mapped_world_height = viewport.height as f64 * MAP_SCALE_FACTOR as f64;

    (visible_world_height / mapped_world_height).max(1.0)
}

fn expanded_bbox_for_loading(bbox: &GeoBBox, viewport: Size) -> GeoBBox {
    let scale = viewport_geo_overscan(viewport) * CHUNK_LOAD_OVERSCAN;
    let center_lat = (bbox.min_lat + bbox.max_lat) * 0.5;
    let center_lon = (bbox.min_lon + bbox.max_lon) * 0.5;
    let half_lat_span = (bbox.max_lat - bbox.min_lat) * 0.5 * scale;
    let half_lon_span = (bbox.max_lon - bbox.min_lon) * 0.5 * scale;

    GeoBBox {
        min_lat: center_lat - half_lat_span,
        max_lat: center_lat + half_lat_span,
        min_lon: center_lon - half_lon_span,
        max_lon: center_lon + half_lon_span,
    }
}

#[derive(Default)]
pub struct RenderState {
    pub chunks: Vec<ChunkData<MapElement>>,
    pub map_elements: Vec<MapElement>,
    pub mesh_container: Vec<OwnedMeshData>,
    pub bbox: GeoBBox,
    pub load_bbox: GeoBBox,
}

impl RenderState {
    pub fn set_bbox(&mut self, centro_lat: f64, centro_lon: f64, raggio_metri: f64) {
        self.set_bbox_for_viewport(centro_lat, centro_lon, raggio_metri, Size::new(1, 1));
    }

    pub fn set_bbox_for_viewport(
        &mut self,
        centro_lat: f64,
        centro_lon: f64,
        raggio_metri: f64,
        viewport: Size,
    ) {
        self.bbox = bbox_for_viewport(centro_lat, centro_lon, raggio_metri, viewport);
        self.load_bbox = expanded_bbox_for_loading(&self.bbox, viewport);
    }

    pub fn reload_chunks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cfg = ChunkConfig {
            chunk_size_m: 2000.0,
        };

        let now = Instant::now();

        let chunks = load_chunks_for_bbox::<MapElement>("chunks", &self.load_bbox, cfg)?;

        let elapsed = now.elapsed();
        println!("Tempo di caricamento chunk: {:?}", elapsed);

        self.chunks = chunks;

        Ok(())
    }

    pub fn reload_map_elements(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let chunk_bboxes = self.chunks.iter().map(|c| c.bbox()).collect::<Vec<_>>();
        let elementi_mappa = self
            .chunks
            .iter()
            .flat_map(|e| e.data.iter())
            .collect::<Vec<_>>();

        let mut elementi_mappa = elementi_mappa
            .iter()
            .map(|e| e.primitive.clone())
            .collect::<Vec<_>>();

        if SHOW_CHUNK_BORDERS {
            for (i, cb) in chunk_bboxes.into_iter().enumerate() {
                let verts = vec![
                    (cb.min_lat, cb.min_lon),
                    (cb.min_lat, cb.max_lon),
                    (cb.max_lat, cb.max_lon),
                    (cb.max_lat, cb.min_lon),
                    (cb.min_lat, cb.min_lon),
                ];
                elementi_mappa.push(MapElement {
                    id: -1 - (i as i64),
                    vertices: verts,
                    inner_rings: Vec::new(),
                    element_type: ElementType::ChunkBorder,
                });
            }
        }

        let elapsed = now.elapsed();
        println!("Tempo di conversione elementi mappa: {:?}", elapsed);

        self.map_elements = elementi_mappa;
        Ok(())
    }

    pub fn reload_mesh_container<D: DrawTarget<Color = Rgb565> + OriginDimensions>(
        &mut self,
        framebuffer: &mut D,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Crea i parametri di conversione
        // Usiamo z diversi per priorità: priorità più alta = z più alto (più vicino alla camera)
        // Questo assicura che gli edifici (priorità 2) siano sopra le aree (priorità 0-1)
        // e le strade (priorità 3) siano sopra gli edifici
        // z_spacing più grande per garantire che gli edifici siano sempre visibili
        let params = ConversionParams {
            bbox: self.bbox.clone(),
            width: framebuffer.size().width,
            height: framebuffer.size().height,
            scale_factor: MAP_SCALE_FACTOR,
            z_base: 0.0,     // Base z per elementi con priorità 0
            z_spacing: 0.01, // Spaziatura tra i livelli di priorità (più grande per garantire visibilità)
            force_wireframe: false,
        };

        let mesh_container = converti_a_mesh(&self.map_elements, params);

        self.mesh_container = mesh_container;
        Ok(())
    }

    /// Renderizza la mappa degli elementi ad alto livello nel raggio specificato
    pub fn renderizza_mappa<D: GFX2DCanvas<Color = embedded_graphics_core::pixelcolor::Rgb565>>(
        &self,
        framebuffer: &mut D,
    ) -> Result<(), DrawError> {
        // Crea l'engine 3D
        let mut engine = K3dengine::new(framebuffer.limit().x as u16, framebuffer.limit().y as u16);

        // Configura la camera per vedere gli oggetti a z=0
        // Dopo la trasformazione view, z diventa la distanza dalla camera
        // Se camera è a z=-5 e oggetti a z=0, dopo view gli oggetti sono a z=5
        engine.camera.near = 0.1;
        engine.camera.far = 100.0;

        // Posiziona la camera più vicina per zoomare sulla mappa
        // Distanza più piccola = zoom maggiore
        engine
            .camera
            .set_position(Point3::new(0.0, 0.0, CAMERA_DISTANCE));
        engine.camera.set_target(Point3::new(0.0, 0.0, 0.0));
        // FOV più stretto per zoomare di più (30 gradi invece di 90)
        engine.camera.set_fovy(CAMERA_FOVY);

        // Usa rendering_adapter per creare le mesh

        // Renderizza tutte le mesh
        // L'API si aspetta IntoIterator<Item = &K3dMesh>, quindi passiamo &meshes
        let mut primitive_count = 0;

        let meshes = self
            .mesh_container
            .iter()
            .map(|mesh_data_item| mesh_data_item.to_kmesh());

        engine.render(meshes, |p| {
            primitive_count += 1;
            let e = draw(&p, framebuffer);

            /*if let Err(e) = e {
                error!("Error drawing primitive: {:?} {:?}", p, e);
            }*/
        });
        info!("Renderizzati {} primitivi", primitive_count);

        Ok(())
    }
}

pub fn filtra_map_elements(elementi_mappa: Vec<MapElement>, bbox: &GeoBBox) -> Vec<MapElement> {
    elementi_mappa
        .iter()
        .filter(|e| bbox.intersects(&e.bbox()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox_size_in_meters(bbox: &GeoBBox, center_lat: f64) -> (f64, f64) {
        let height_m = (bbox.max_lat - bbox.min_lat) * 111000.0;
        let width_m =
            (bbox.max_lon - bbox.min_lon) * 111000.0 * center_lat.to_radians().cos().abs();
        (width_m, height_m)
    }

    #[test]
    fn viewport_wide_expands_horizontal_span() {
        let center_lat = 45.47362;
        let bbox = bbox_for_viewport(center_lat, 9.24919, 200.0, Size::new(1920, 1080));
        let (width_m, height_m) = bbox_size_in_meters(&bbox, center_lat);

        assert!(width_m > height_m);
        assert!((width_m / height_m - (1920.0 / 1080.0)).abs() < 0.01);
    }

    #[test]
    fn viewport_tall_expands_vertical_span() {
        let center_lat = 45.47362;
        let bbox = bbox_for_viewport(center_lat, 9.24919, 200.0, Size::new(1080, 1920));
        let (width_m, height_m) = bbox_size_in_meters(&bbox, center_lat);

        assert!(height_m > width_m);
        assert!((width_m / height_m - (1080.0 / 1920.0)).abs() < 0.01);
    }

    #[test]
    fn loading_bbox_expands_to_camera_visible_area() {
        let center_lat = 45.47362;
        let viewport = Size::new(1920, 1080);
        let bbox = bbox_for_viewport(center_lat, 9.24919, 200.0, viewport);
        let load_bbox = expanded_bbox_for_loading(&bbox, viewport);
        let (width_m, height_m) = bbox_size_in_meters(&bbox, center_lat);
        let (load_width_m, load_height_m) = bbox_size_in_meters(&load_bbox, center_lat);

        assert!(load_width_m > width_m);
        assert!(load_height_m > height_m);
        assert!(load_height_m / height_m >= viewport_geo_overscan(viewport));
    }
}

pub fn filtra_raw_osm_data(
    accumulator: RawOsmData,
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
) -> RawOsmData {
    // 1) Tieni solo i nodi entro il raggio
    let nodes: Vec<_> = accumulator
        .nodes
        .into_iter()
        .filter(|n| entro_raggio(n.lat, n.lon, centro_lat, centro_lon, raggio_metri))
        .collect();

    let node_ids: HashSet<i64> = nodes.iter().map(|n| n.id).collect();

    // 2) Tieni solo le ways che hanno almeno un nodo nel raggio,
    //    e clippa i node_refs ai soli nodi rimasti
    let ways: Vec<_> = accumulator
        .ways
        .into_iter()
        .filter_map(|mut w| {
            w.node_refs.retain(|id| node_ids.contains(id));
            (!w.node_refs.is_empty()).then_some(w)
        })
        .collect();

    let way_ids: HashSet<i64> = ways.iter().map(|w| w.id).collect();

    // 3) Tieni solo le relazioni multipolygon che referenziano almeno una way rimasta.
    //    (I membri way che non esistono più vengono scartati.)
    let relations: Vec<_> = accumulator
        .relations
        .into_iter()
        .filter_map(|mut r| {
            let mut has_any_way = false;
            r.members.retain(|m| {
                if m.member_type == RelationMemberType::Way {
                    let keep = way_ids.contains(&m.member_id);
                    if keep {
                        has_any_way = true;
                    }
                    keep
                } else {
                    true
                }
            });
            has_any_way.then_some(r)
        })
        .collect();

    RawOsmData {
        nodes,
        ways,
        relations,
    }
}
