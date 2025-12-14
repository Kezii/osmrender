use osmpbf::{Element, ElementReader};

use osmrender::chunk_manager::{ChunkConfig, save_chunks};
use osmrender::converter::{NodeData, RelationData, RelationMember, RelationMemberType, WayData};
use osmrender::spatial_index::build_spatial_index;

#[derive(Clone)]
struct Accumulator {
    nodes: Vec<NodeData>,
    ways: Vec<WayData>,
    relations: Vec<RelationData>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let input = "nord-ovest-251207.osm.pbf";
    let out_dir = "chunks";
    let cfg = ChunkConfig { chunk_size_m: 10000.0 };

    println!("Input: {}", input);
    println!("Output dir: {}", out_dir);
    println!("Chunk size (m): {}", cfg.chunk_size_m);

    println!("--- Fase 1: Lettura e raccolta elementi (in parallelo) ---");

    let reader = ElementReader::from_path(input)?;
    let accumulator = reader.par_map_reduce(
        |element| {
            match element {
                Element::DenseNode(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    Accumulator {
                        nodes: vec![NodeData {
                            id: node.id(),
                            lat: node.lat(),
                            lon: node.lon(),
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
                    Accumulator {
                        nodes: vec![NodeData {
                            id: node.id(),
                            lat: node.lat(),
                            lon: node.lon(),
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
                    Accumulator {
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

                        Accumulator {
                            nodes: Vec::new(),
                            ways: Vec::new(),
                            relations: vec![RelationData {
                                id: rel.id(),
                                tags,
                                members,
                            }],
                        }
                    } else {
                        Accumulator {
                            nodes: Vec::new(),
                            ways: Vec::new(),
                            relations: Vec::new(),
                        }
                    }
                }
            }
        },
        || Accumulator {
            nodes: Vec::new(),
            ways: Vec::new(),
            relations: Vec::new(),
        },
        |a, b| Accumulator {
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

    let nodes_data = accumulator.nodes;
    let ways_data = accumulator.ways;
    let relations_data = accumulator.relations;

    // Rappresentazione intermedia: primitive OSM indicizzate per posizione (griglia lat/lon)
    // Nota: per ways/relations la posizione è una "rappresentativa" (centro bbox dei nodi).
    println!("--- Fase 1b: Indicizzazione spaziale primitive OSM ---");

    let spatial = build_spatial_index(nodes_data, ways_data, relations_data);
    println!("Primitive posizionate: {}", spatial.len());

    println!("--- Salvataggio chunk ---");
    save_chunks(&spatial, &out_dir, cfg)?;
    println!("Done.");

    Ok(())
}


