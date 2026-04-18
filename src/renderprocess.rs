use crate::chunk_manager::{ChunkConfig, ChunkData, GeoBBox, load_chunks_for_bbox};
use crate::imageframebuffer::ImageFramebuffer;
use crate::map_elements::{ElementType, MapElement};
use crate::raw_osm_reader::{RawOsmData, RelationMemberType};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, Size};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use image::RgbImage;
use log::error;
use std::collections::HashSet;
use std::time::Instant;

use crate::render;

/// Switch per abilitare il rendering dei bordi dei chunk (overlay debug).
const SHOW_CHUNK_BORDERS: bool = true;

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

#[derive(Default)]
pub struct RenderState {
    pub chunks: Vec<ChunkData<MapElement>>,
    pub bbox: GeoBBox,
}

impl RenderState {
    pub fn set_bbox(&mut self, centro_lat: f64, centro_lon: f64, raggio_metri: f64) {
        let bbox = GeoBBox {
            min_lat: centro_lat - raggio_metri / 111000.0,
            max_lat: centro_lat + raggio_metri / 111000.0,
            min_lon: centro_lon - raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
            max_lon: centro_lon + raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
        };

        self.bbox = bbox;
    }

    pub fn reload_chunks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cfg = ChunkConfig {
            chunk_size_m: 20000.0,
        };

        let now = Instant::now();

        let chunks = load_chunks_for_bbox::<MapElement>("chunks", &self.bbox, cfg)?;

        let elapsed = now.elapsed();
        println!("Tempo di caricamento chunk: {:?}", elapsed);

        self.chunks = chunks;
        Ok(())
    }

    /// Stampa gli elementi OSM solo se sono entro un raggio specificato
    pub fn stampa_elementi_in_raggio<D: DrawTarget<Color = Rgb565> + OriginDimensions>(
        &mut self,
        framebuffer: &mut D,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        <D as DrawTarget>::Error: std::fmt::Debug,
    {
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

        let now = Instant::now();
        // Renderizza la mappa usando gli elementi ad alto livello
        let e = render::renderizza_mappa(&elementi_mappa, &self.bbox, framebuffer);

        if let Err(e) = e {
            error!("Error rendering mappa: {:?}", e);
        }

        let elapsed = now.elapsed();
        println!("Tempo di rendering mappa: {:?}", elapsed);
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
