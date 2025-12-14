use std::collections::HashMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::converter::{NodeData, RelationData, RelationMemberType, SpatialNodeData, WayData};

/// Primitive OSM “grezze” (non ancora convertite in MapElement).
#[derive(Debug, Clone, Serialize, Deserialize,bincode::Encode, bincode::Decode)]
pub enum OsmPrimitive {
    Node(SpatialNodeData),
    Way(WayData),
    /// Tipicamente `type=multipolygon` (ma non lo forziamo qui).
    Relation(RelationData),
}

/// Una primitive OSM associata ad una posizione rappresentativa (lat/lon).
#[derive(Debug, Clone, Serialize, Deserialize,bincode::Encode, bincode::Decode)]
pub struct PositionedPrimitive {
    pub lat: f64,
    pub lon: f64,
    pub primitive: OsmPrimitive,
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

    fn add(&mut self, lat: f64, lon: f64) {
        if !lat.is_finite() || !lon.is_finite() {
            return;
        }
        self.min_lat = self.min_lat.min(lat);
        self.max_lat = self.max_lat.max(lat);
        self.min_lon = self.min_lon.min(lon);
        self.max_lon = self.max_lon.max(lon);
        self.count += 1;
    }

    fn center(&self) -> Option<(f64, f64)> {
        if self.count == 0 || !self.min_lat.is_finite() || !self.min_lon.is_finite() {
            return None;
        }
        Some(((self.min_lat + self.max_lat) * 0.5, (self.min_lon + self.max_lon) * 0.5))
    }
}

fn way_rep_position(way: &WayData, node_pos: &HashMap<i64, (f64, f64)>) -> Option<(f64, f64)> {
    let mut bbox = BBox::new();
    for node_id in &way.node_refs {
        if let Some(&(lat, lon)) = node_pos.get(node_id) {
            bbox.add(lat, lon);
        }
    }
    bbox.center()
}

fn relation_rep_position(
    rel: &RelationData,
    ways_by_id: &HashMap<i64, &WayData>,
    node_pos: &HashMap<i64, (f64, f64)>,
) -> Option<(f64, f64)> {
    let mut bbox = BBox::new();

    for m in &rel.members {
        match m.member_type {
            RelationMemberType::Way => {
                if let Some(way) = ways_by_id.get(&m.member_id) {
                    for node_id in &way.node_refs {
                        if let Some(&(lat, lon)) = node_pos.get(node_id) {
                            bbox.add(lat, lon);
                        }
                    }
                }
            }
            RelationMemberType::Node => {
                if let Some(&(lat, lon)) = node_pos.get(&m.member_id) {
                    bbox.add(lat, lon);
                }
            }
            RelationMemberType::Relation => {
                // Per ora ignoriamo relazioni annidate: se serve, si può fare una risoluzione ricorsiva.
            }
        }
    }

    bbox.center()
}

/// Costruisce una rappresentazione intermedia “indicizzata per posizione”:
/// - **Node**: piazzato sul suo lat/lon.
/// - **Way**: piazzata sul centro del bbox dei nodi referenziati.
/// - **Relation**: piazzata sul centro del bbox dei nodi delle ways membro (fallback su node-members).
pub fn build_spatial_index(
    nodes: Vec<NodeData>,
    ways: Vec<WayData>,
    relations: Vec<RelationData>,
) -> Vec<PositionedPrimitive> {

    let node_pos: HashMap<i64, (f64, f64)> = nodes
        .par_iter()
        .map(|n| (n.id, (n.lat, n.lon)))
        .collect();

    println!("node index done");

    let mut spatial = Vec::with_capacity(nodes.len() + ways.len() + relations.len());
    
    spatial.par_extend(nodes.into_par_iter().map(|n| PositionedPrimitive {
        lat: n.lat,
        lon: n.lon,
        primitive: OsmPrimitive::Node(SpatialNodeData { id: n.id, tags: n.tags.clone() }),
    }));


    let ways_by_id: HashMap<i64, &WayData> = ways.par_iter().map(|w| (w.id, w)).collect();

    let rel_positions: Vec<Option<(f64, f64)>> = relations
        .par_iter()
        .map(|r| relation_rep_position(r, &ways_by_id, &node_pos))
        .collect();

    // Ways
    spatial.par_extend(ways.into_par_iter().filter_map(|w| way_rep_position(&w, &node_pos).map(|(lat, lon)| PositionedPrimitive {
        lat,
        lon,
        primitive: OsmPrimitive::Way(w),
    })));

    // Relations (multipolygon o altro): posizione rappresentativa via membri.
    spatial.par_extend(relations.into_par_iter().zip(rel_positions.into_par_iter()).filter_map(|(r, pos)| pos.map(|(lat, lon)| PositionedPrimitive {
        lat,
        lon,
        primitive: OsmPrimitive::Relation(r),
    })));

    spatial
}


