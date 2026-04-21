use osmpbf::{Element, ElementReader};
use serde::{Deserialize, Serialize};

use crate::GeoPos;

/// Dati di un nodo OSM
#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub struct NodeData {
    pub id: i64,
    pub pos: GeoPos,
    pub tags: Vec<(String, String)>,
}

/// Dati di una way OSM
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct WayData {
    pub id: i64,
    pub node_refs: Vec<i64>,
    pub tags: Vec<(String, String)>,
}

/// Dati di una relazione OSM
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct RelationData {
    pub id: i64,
    pub tags: Vec<(String, String)>,
    pub members: Vec<RelationMember>,
}

/// Membro di una relazione OSM
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct RelationMember {
    pub member_type: RelationMemberType,
    pub member_id: i64,
    pub role: String,
}

/// Tipo di membro di una relazione
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum RelationMemberType {
    Node,
    Way,
    Relation,
}

#[derive(Clone)]
pub struct RawOsmData {
    pub nodes: Vec<NodeData>,
    pub ways: Vec<WayData>,
    pub relations: Vec<RelationData>,
}

pub fn read_raw_osm_file(path: &str) -> Result<RawOsmData, Box<dyn std::error::Error>> {
    let reader = ElementReader::from_path(path)?;
    let accumulator = reader.par_map_reduce(
        |element| {
            match element {
                Element::DenseNode(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    RawOsmData {
                        nodes: vec![NodeData {
                            id: node.id(),
                            pos: GeoPos::new(node.lat(), node.lon()),
                            tags,
                        }],
                        ways: Vec::new(),
                        relations: Vec::new(),
                    }
                }
                Element::Node(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    RawOsmData {
                        nodes: vec![NodeData {
                            id: node.id(),
                            pos: GeoPos::new(node.lat(), node.lon()),
                            tags,
                        }],
                        ways: Vec::new(),
                        relations: Vec::new(),
                    }
                }
                Element::Way(way) => {
                    let node_refs: Vec<i64> = way.refs().collect();
                    let tags: Vec<(String, String)> = way
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    RawOsmData {
                        nodes: Vec::new(),
                        ways: vec![WayData {
                            id: way.id(),
                            node_refs,
                            tags,
                        }],
                        relations: Vec::new(),
                    }
                }
                Element::Relation(rel) => {
                    let tags: Vec<(String, String)> = rel
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();

                    // Verifica se è un multipolygon
                    let is_multipolygon =
                        tags.iter().any(|(k, v)| k == "type" && v == "multipolygon");

                    if is_multipolygon {
                        let members: Vec<RelationMember> = rel
                            .members()
                            .map(|m| {
                                let member_type = match m.member_type {
                                    osmpbf::RelMemberType::Node => RelationMemberType::Node,
                                    osmpbf::RelMemberType::Way => RelationMemberType::Way,
                                    osmpbf::RelMemberType::Relation => RelationMemberType::Relation,
                                };
                                let role = m.role().unwrap_or("").to_string();
                                RelationMember {
                                    member_type,
                                    member_id: m.member_id,
                                    role,
                                }
                            })
                            .collect();

                        RawOsmData {
                            nodes: Vec::new(),
                            ways: Vec::new(),
                            relations: vec![RelationData {
                                id: rel.id(),
                                tags,
                                members,
                            }],
                        }
                    } else {
                        RawOsmData {
                            nodes: Vec::new(),
                            ways: Vec::new(),
                            relations: Vec::new(),
                        }
                    }
                }
            }
        },
        || RawOsmData {
            nodes: Vec::new(),
            ways: Vec::new(),
            relations: Vec::new(),
        },
        |a, b| RawOsmData {
            nodes: {
                let mut combined = a.nodes;
                combined.extend(b.nodes);
                combined
            },
            ways: {
                let mut combined = a.ways;
                combined.extend(b.ways);
                combined
            },
            relations: {
                let mut combined = a.relations;
                combined.extend(b.relations);
                combined
            },
        },
    )?;

    Ok(accumulator)
}
