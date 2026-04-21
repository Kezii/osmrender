use std::collections::HashMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    GeoPos,
    chunk_manager::GeoBBox,
    converter::SpatialNodeData,
    raw_osm_reader::{NodeData, RelationData, RelationMemberType, WayData},
};

/// Primitive OSM “grezze” (non ancora convertite in MapElement).
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum OsmPrimitive {
    Node(SpatialNodeData),
    Way(WayData),
    /// Tipicamente `type=multipolygon` (ma non lo forziamo qui).
    Relation(RelationData),
}

/// Una primitive OSM associata alla sua **bounding box geografica**.
#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct PositionedPrimitive {
    pub bbox: GeoBBox,
    pub primitive: OsmPrimitive,
}

impl PositionedPrimitive {
    /// Chiave stabile per deduplicare primitive che possono finire in più chunk.
    pub fn osm_key(&self) -> (u8, i64) {
        match &self.primitive {
            OsmPrimitive::Node(n) => (0, n.id),
            OsmPrimitive::Way(w) => (1, w.id),
            OsmPrimitive::Relation(r) => (2, r.id),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BBox {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
    count: usize,
}

impl BBox {
    fn new() -> Self {
        Self {
            min_lat: f64::INFINITY,
            max_lat: f64::NEG_INFINITY,
            min_lon: f64::INFINITY,
            max_lon: f64::NEG_INFINITY,
            count: 0,
        }
    }

    fn add(&mut self, pos: GeoPos) {
        if !pos.lat().is_finite() || !pos.lon().is_finite() {
            return;
        }
        self.min_lat = self.min_lat.min(pos.lat());
        self.max_lat = self.max_lat.max(pos.lat());
        self.min_lon = self.min_lon.min(pos.lon());
        self.max_lon = self.max_lon.max(pos.lon());
        self.count += 1;
    }

    fn into_geo_bbox(self) -> Option<GeoBBox> {
        if self.count == 0
            || !self.min_lat.is_finite()
            || !self.max_lat.is_finite()
            || !self.min_lon.is_finite()
            || !self.max_lon.is_finite()
        {
            return None;
        }
        Some(GeoBBox {
            min: GeoPos::new(self.min_lat, self.min_lon),
            max: GeoPos::new(self.max_lat, self.max_lon),
        })
    }
}

fn way_bbox(way: &WayData, node_pos: &HashMap<i64, GeoPos>) -> Option<GeoBBox> {
    let mut bbox = BBox::new();
    for node_id in &way.node_refs {
        if let Some(&pos) = node_pos.get(node_id) {
            bbox.add(pos);
        }
    }
    bbox.into_geo_bbox()
}

fn relation_bbox(
    rel: &RelationData,
    ways_by_id: &HashMap<i64, &WayData>,
    node_pos: &HashMap<i64, GeoPos>,
) -> Option<GeoBBox> {
    let mut bbox = BBox::new();

    for m in &rel.members {
        match m.member_type {
            RelationMemberType::Way => {
                if let Some(way) = ways_by_id.get(&m.member_id) {
                    for node_id in &way.node_refs {
                        if let Some(&pos) = node_pos.get(node_id) {
                            bbox.add(pos);
                        }
                    }
                }
            }
            RelationMemberType::Node => {
                if let Some(&pos) = node_pos.get(&m.member_id) {
                    bbox.add(pos);
                }
            }
            RelationMemberType::Relation => {
                // Per ora ignoriamo relazioni annidate: se serve, si può fare una risoluzione ricorsiva.
            }
        }
    }

    bbox.into_geo_bbox()
}

/// Costruisce una rappresentazione intermedia “indicizzata per posizione”:
/// - **Node**: bbox puntiforme (min=max=lat/lon).
/// - **Way**: bbox dei nodi referenziati.
/// - **Relation**: bbox dei nodi delle ways membro (fallback su node-members).
pub fn build_spatial_index(
    nodes: Vec<NodeData>,
    ways: Vec<WayData>,
    relations: Vec<RelationData>,
) -> Vec<PositionedPrimitive> {
    let node_pos: HashMap<i64, GeoPos> = nodes.par_iter().map(|n| (n.id, n.pos)).collect();

    println!("node index done");

    let mut spatial = Vec::with_capacity(nodes.len() + ways.len() + relations.len());

    spatial.par_extend(nodes.into_par_iter().map(|n| PositionedPrimitive {
        bbox: GeoBBox {
            min: GeoPos::new(n.pos.lat(), n.pos.lon()),
            max: GeoPos::new(n.pos.lat(), n.pos.lon()),
        },
        primitive: OsmPrimitive::Node(SpatialNodeData {
            id: n.id,
            tags: n.tags.clone(),
        }),
    }));

    let ways_by_id: HashMap<i64, &WayData> = ways.par_iter().map(|w| (w.id, w)).collect();

    let rel_bboxes: Vec<Option<GeoBBox>> = relations
        .par_iter()
        .map(|r| relation_bbox(r, &ways_by_id, &node_pos))
        .collect();

    // Ways
    spatial.par_extend(ways.into_par_iter().filter_map(|w| {
        way_bbox(&w, &node_pos).map(|bbox| PositionedPrimitive {
            bbox,
            primitive: OsmPrimitive::Way(w),
        })
    }));

    // Relations (multipolygon o altro): bbox via membri.
    spatial.par_extend(
        relations
            .into_par_iter()
            .zip(rel_bboxes.into_par_iter())
            .filter_map(|(r, bbox)| {
                bbox.map(|bbox| PositionedPrimitive {
                    bbox,
                    primitive: OsmPrimitive::Relation(r),
                })
            }),
    );

    spatial
}
