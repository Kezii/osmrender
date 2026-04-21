use crate::chunk_manager::{ChunkConfig, ChunkData, GeoBBoxable, load_chunks_for_bbox};
use crate::map_elements::{ElementType, MapElement};
use crate::raw_osm_reader::{RawOsmData, RelationMemberType};
use crate::rendering_adapter::{MapToMeshConversionParams, OwnedMeshData};
use crate::{GeoBBox, GeoPos};
use embedded_gfx::K3dengine;
use embedded_gfx::canvas::{DrawError, GFX2DCanvas};
use embedded_gfx::draw::draw;
use embedded_graphics::prelude::Size;
use itertools::Itertools;
use nalgebra::{Point3, Vector2};
use std::collections::HashSet;

/// Switch per abilitare il rendering dei bordi dei chunk (overlay debug).
const SHOW_CHUNK_BORDERS: bool = true;
pub const MAP_SCALE_FACTOR: f32 = 0.001;
pub const CAMERA_DISTANCE: f32 = 2.0;
//pub const CAMERA_FOVY: f32 = std::f32::consts::PI / 6.0;
const CHUNK_LOAD_OVERSCAN: f64 = 1.05;

/// Calcola la distanza in metri tra due coordinate geografiche usando la formula di Haversine
fn distanza_geografica(point1: GeoPos, point2: GeoPos) -> f64 {
    const R: f64 = 6371000.0; // Raggio della Terra in metri

    let d_lat = (point2.lat() - point1.lat()).to_radians();
    let d_lon = (point2.lon() - point1.lon()).to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + point1.lat().to_radians().cos()
            * point2.lat().to_radians().cos()
            * (d_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().asin();

    R * c
}

/// Verifica se un punto è entro un raggio specificato da un punto centrale
fn entro_raggio(pos: GeoPos, centro: GeoPos, raggio_metri: f64) -> bool {
    distanza_geografica(pos, centro) <= raggio_metri
}

fn bbox_for_viewport(centro: GeoPos, raggio_metri: f64, viewport: Size) -> GeoBBox {
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
    let meters_per_lon_degree = (111000.0 * centro.lat().to_radians().cos().abs()).max(1.0);
    let lon_delta = half_width_m / meters_per_lon_degree;

    GeoBBox {
        min: GeoPos::new(centro.lat() - lat_delta, centro.lon() - lon_delta),
        max: GeoPos::new(centro.lat() + lat_delta, centro.lon() + lon_delta),
    }
}

pub struct RenderState {
    pub chunks: Vec<ChunkData<MapElement>>,
    pub mesh_container: Vec<OwnedMeshData>,
    pub viewport_size: Size,
    pub camera_fovy: f32,
    pub spawn_point: GeoPos,
    pub current_center: GeoPos,
}

impl RenderState {
    pub fn zoom(&mut self, zoom_factor: f32) {
        self.camera_fovy *= zoom_factor;
    }

    /// Returns the visible span along the map plane's `y` axis where the camera
    /// frustum intersects `z = 0`, expressed in renderer world units.
    fn visible_world_height_at_z0(&self) -> f64 {
        2.0 * CAMERA_DISTANCE as f64 * (self.camera_fovy as f64 / 2.0).tan()
    }

    /// Converts the camera-visible area into real-world meters for the current
    /// viewport, preserving the viewport aspect ratio.
    fn visible_meters_for_viewport(&self, viewport: Size) -> (f64, f64) {
        let aspect_ratio = if viewport.height == 0 {
            1.0
        } else {
            viewport.width as f64 / viewport.height as f64
        }
        .max(f64::EPSILON);

        let visible_height_m = self.visible_world_height_at_z0() / MAP_SCALE_FACTOR as f64;
        let visible_width_m = visible_height_m * aspect_ratio;

        (visible_width_m, visible_height_m)
    }

    pub fn viewport_geo_overscan(&self, viewport: Size) -> f64 {
        if viewport.height == 0 {
            return 1.0;
        }

        let visible_world_height = self.visible_world_height_at_z0();
        let mapped_world_height = viewport.height as f64 * MAP_SCALE_FACTOR as f64;

        (visible_world_height / mapped_world_height).max(1.0)
    }

    fn expanded_bbox_for_loading(&self, bbox: &GeoBBox, viewport: Size) -> GeoBBox {
        let scale = self.viewport_geo_overscan(viewport) * CHUNK_LOAD_OVERSCAN;
        let center_lat = (bbox.min.lat() + bbox.max.lat()) * 0.5;
        let center_lon = (bbox.min.lon() + bbox.max.lon()) * 0.5;
        let half_lat_span = (bbox.max.lat() - bbox.min.lat()) * 0.5 * scale;
        let half_lon_span = (bbox.max.lon() - bbox.min.lon()) * 0.5 * scale;

        GeoBBox {
            min: GeoPos::new(center_lat - half_lat_span, center_lon - half_lon_span),
            max: GeoPos::new(center_lat + half_lat_span, center_lon + half_lon_span),
        }
    }

    // the bbox in geo coordinates
    pub fn get_geo_bbox(&self) -> GeoBBox {
        let (visible_width_m, visible_height_m) =
            self.visible_meters_for_viewport(self.viewport_size);
        let radius_m = visible_width_m.min(visible_height_m) * 0.5;

        bbox_for_viewport(self.current_center, radius_m, self.viewport_size)
    }

    /// computes camera x y from the current center and the viewport
    /// so the center in world units
    pub fn get_world_center(&self) -> (f32, f32) {
        let (north_m, east_m) = self.spawn_point.offset_in_meters(self.current_center);
        let camera_x = (east_m * MAP_SCALE_FACTOR as f64) as f32;
        let camera_y = (north_m * MAP_SCALE_FACTOR as f64) as f32;

        (camera_x, camera_y)
    }

    /// Returns the visible bbox on the `z = 0` map plane in renderer world
    /// coordinates as `(min_x, min_y, max_x, max_y)`.
    pub fn get_world_bbox(&self) -> (Vector2<f32>, Vector2<f32>) {
        let (center_x, center_y) = self.get_world_center();
        let aspect_ratio = if self.viewport_size.height == 0 {
            1.0
        } else {
            self.viewport_size.width as f32 / self.viewport_size.height as f32
        }
        .max(f32::EPSILON);

        let visible_height = self.visible_world_height_at_z0() as f32;
        let visible_width = visible_height * aspect_ratio;
        let half_width = visible_width * 0.5;
        let half_height = visible_height * 0.5;

        (
            Vector2::new(center_x - half_width, center_y - half_height),
            Vector2::new(center_x + half_width, center_y + half_height),
        )
    }

    pub fn reload_chunks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let load_bbox = self.expanded_bbox_for_loading(&self.get_geo_bbox(), self.viewport_size);

        let cfg = ChunkConfig {
            chunk_size_m: 2000.0,
        };

        let chunks = load_chunks_for_bbox::<MapElement>("chunks", &load_bbox, cfg)?;

        self.chunks = chunks;

        Ok(())
    }

    pub fn get_chunk_borders(&self) -> impl Iterator<Item = MapElement> {
        self.chunks
            .iter()
            .map(|c| c.bbox())
            .enumerate()
            .map(|(i, cb)| {
                let verts = vec![
                    GeoPos::new(cb.min.lat(), cb.min.lon()),
                    GeoPos::new(cb.min.lat(), cb.max.lon()),
                    GeoPos::new(cb.max.lat(), cb.max.lon()),
                    GeoPos::new(cb.max.lat(), cb.min.lon()),
                    GeoPos::new(cb.min.lat(), cb.min.lon()),
                ];
                MapElement {
                    id: -1 - (i as i64),
                    vertices: verts,
                    inner_rings: Vec::new(),
                    element_type: ElementType::ChunkBorder,
                }
            })
    }

    pub fn map_to_mesh(&mut self, spawn_point: GeoPos) -> Result<(), Box<dyn std::error::Error>> {
        let params = MapToMeshConversionParams {
            center_offset: spawn_point,
            scale_factor: MAP_SCALE_FACTOR as f64,
            z_base: 0.0,                        // Base z per elementi con priorità 0
            z_spacing: 0.01 * MAP_SCALE_FACTOR, // Spaziatura tra i livelli di priorità (più grande per garantire visibilità)
            force_wireframe: false,
        };

        let mut mesh_container: Vec<OwnedMeshData> = self
            .chunks
            .iter()
            .flat_map(|e| e.data.iter())
            .unique_by(|m| m.id)
            .filter_map(|e| e.converti_a_mesh(&params))
            .collect();

        if SHOW_CHUNK_BORDERS {
            mesh_container.extend(
                self.get_chunk_borders()
                    .filter_map(|e| e.converti_a_mesh(&params)),
            );
        }

        mesh_container.sort_by_key(|m| m.priority);

        self.mesh_container = mesh_container;
        Ok(())
    }

    /// Renderizza la mappa degli elementi ad alto livello nel raggio specificato
    pub fn renderizza_mappa<D: GFX2DCanvas<Color = embedded_graphics_core::pixelcolor::Rgb565>>(
        &self,
        framebuffer: &mut D,
    ) -> Result<usize, DrawError> {
        // Crea l'engine 3D
        let mut engine = K3dengine::new(framebuffer.limit().x as u16, framebuffer.limit().y as u16);

        // Configura la camera per vedere gli oggetti a z=0
        // Dopo la trasformazione view, z diventa la distanza dalla camera
        // Se camera è a z=-5 e oggetti a z=0, dopo view gli oggetti sono a z=5
        engine.camera.near = 0.1;
        engine.camera.far = 100.0;

        // Posiziona la camera più vicina per zoomare sulla mappa
        // Distanza più piccola = zoom maggiore
        let (camera_x, camera_y) = self.get_world_center();
        engine
            .camera
            .set_position(Point3::new(camera_x, camera_y, CAMERA_DISTANCE));
        engine
            .camera
            .set_target(Point3::new(camera_x, camera_y, 0.0));
        // FOV più stretto per zoomare di più (30 gradi invece di 90)
        engine.camera.set_fovy(self.camera_fovy);

        // Usa rendering_adapter per creare le mesh

        // Renderizza tutte le mesh
        // L'API si aspetta IntoIterator<Item = &K3dMesh>, quindi passiamo &meshes
        let mut primitive_count = 0;

        let current_bbox = self.get_geo_bbox();

        let meshes = self
            .mesh_container
            .iter()
            .filter(|mesh_data_item| current_bbox.intersects(&mesh_data_item.bbox))
            .map(|mesh_data_item| mesh_data_item.to_kmesh());

        engine.render(meshes, |p| {
            primitive_count += 1;
            draw(&p, framebuffer).ok();

            /*if let Err(e) = e {
                error!("Error drawing primitive: {:?} {:?}", p, e);
            }*/
        });

        Ok(primitive_count)
    }
}

pub fn filtra_map_elements(elementi_mappa: Vec<MapElement>, bbox: &GeoBBox) -> Vec<MapElement> {
    elementi_mappa
        .iter()
        .filter(|e| bbox.intersects(&e.bbox()))
        .cloned()
        .collect()
}

pub fn filtra_raw_osm_data(
    accumulator: RawOsmData,
    centro: GeoPos,
    raggio_metri: f64,
) -> RawOsmData {
    // 1) Tieni solo i nodi entro il raggio
    let nodes: Vec<_> = accumulator
        .nodes
        .into_iter()
        .filter(|n| entro_raggio(n.pos, centro, raggio_metri))
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
