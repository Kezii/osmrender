use crate::map_elements::{MapElement, ElementType};
use std::collections::HashMap;

/// Dati di un nodo OSM
#[derive(Clone)]
pub struct NodeData {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: Vec<(String, String)>,
}

/// Dati di una way OSM
#[derive(Clone)]
pub struct WayData {
    pub id: i64,
    pub node_refs: Vec<i64>,
    pub tags: Vec<(String, String)>,
}

/// Converte un nodo OSM in un elemento della mappa
pub fn converti_nodo(
    node_data: &NodeData,
    nodi_nel_raggio: &HashMap<i64, (f64, f64)>,
) -> Option<MapElement> {
    // Verifica che il nodo sia nel raggio
    if !nodi_nel_raggio.contains_key(&node_data.id) {
        return None;
    }

    let name = node_data
        .tags
        .iter()
        .find(|(k, _)| k == "name")
        .map(|(_, v)| v.as_str());

    // Classifica il tipo di nodo in base ai tag
    let elemento = {
        // Controlla se è un albero
        if let Some((_, v)) = node_data.tags.iter().find(|(k, _)| k == "natural") {
            if v == "tree" {
                MapElement {
                    id: node_data.id,
                    vertices: vec![(node_data.lat, node_data.lon)],
                    element_type: ElementType::Albero,
                }
            } else {
                MapElement {
                    id: node_data.id,
                    vertices: vec![(node_data.lat, node_data.lon)],
                    element_type: ElementType::Altro { is_punto: true },
                }
            }
        }
        // Controlla se è un punto di interesse
        else if name.is_some()
            || node_data
                .tags
                .iter()
                .any(|(k, _)| matches!(k.as_str(), "place" | "amenity" | "shop" | "tourism" | "leisure" | "historic"))
        {
            MapElement {
                id: node_data.id,
                vertices: vec![(node_data.lat, node_data.lon)],
                element_type: ElementType::PuntoInteresse {
                    ha_nome: name.is_some(),
                },
            }
        } else {
            MapElement {
                id: node_data.id,
                vertices: vec![(node_data.lat, node_data.lon)],
                element_type: ElementType::Altro { is_punto: true },
            }
        }
    };

    Some(elemento)
}

/// Converte una way OSM in un elemento della mappa
pub fn converti_way(
    way_data: &WayData,
    nodi_nel_raggio: &HashMap<i64, (f64, f64)>,
) -> Option<MapElement> {
    // Raccogli i nodi della way che sono nel raggio
    let vertices: Vec<(f64, f64)> = way_data
        .node_refs
        .iter()
        .filter_map(|&node_id| nodi_nel_raggio.get(&node_id).copied())
        .collect();

    if vertices.is_empty() {
        return None;
    }

    // Classifica il tipo di way in base ai tag
    let elemento = {
        // Controlla prima i waterway (fiumi, canali)
        if let Some((_, v)) = way_data.tags.iter().find(|(k, _)| k == "waterway") {
            match v.as_str() {
                "river" | "stream" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Fiume,
                },
                "canal" | "ditch" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Canale,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Altro { is_punto: false },
                },
            }
        }
        // Controlla le ferrovie
        else if way_data.tags.iter().any(|(k, _)| k == "railway") {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                element_type: ElementType::Ferrovia,
            }
        }
        // Controlla gli edifici
        else if way_data.tags.iter().any(|(k, _)| k == "building") {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                element_type: ElementType::Edificio,
            }
        }
        // Controlla aeroporti (prima di altri landuse)
        else if way_data
            .tags
            .iter()
            .any(|(k, v)| (k == "aeroway" || k == "landuse") && v == "aerodrome")
        {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                element_type: ElementType::Aeroporto,
            }
        }
        // Controlla landuse e leisure (residenziale, commerciale, industriale, agricolo, cimitero, parchi)
        else if let Some((k, v)) = way_data.tags.iter().find(|(k, _)| {
            k == "landuse" || k == "leisure"
        }) {
            match (k.as_str(), v.as_str()) {
                // Parchi e aree verdi (priorità alta)
                ("leisure", "park")
                | ("leisure", "recreation_ground")
                | ("landuse", "recreation_ground")
                | ("landuse", "park")
                | ("leisure", "garden")
                | ("landuse", "grass") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Parco,
                },
                // Altri landuse
                ("landuse", "residential") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Residenziale,
                },
                ("landuse", "commercial") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Commerciale,
                },
                ("landuse", "industrial") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Industriale,
                },
                ("landuse", "farmland") | ("landuse", "farmyard") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Agricolo,
                },
                ("landuse", "cemetery") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Cimitero,
                },
                // Acqua da landuse
                ("landuse", "basin") | ("landuse", "reservoir") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Acqua,
                },
                // Campi sportivi
                ("leisure", "pitch") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::CampoSportivo,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Altro { is_punto: false },
                },
            }
        }
        // Controlla l'acqua da natural (dopo landuse/leisure)
        else if let Some((_, v)) = way_data.tags.iter().find(|(k, _)| k == "natural") {
            match v.as_str() {
                "water" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Acqua,
                },
                "wood" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Foresta,
                },
                "scrub" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Boscaglia,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Altro { is_punto: false },
                },
            }
        }
        // Controlla acqua da leisure (piscine)
        else if let Some((_, v)) = way_data.tags.iter().find(|(k, _)| k == "leisure") {
            match v.as_str() {
                "swimming_pool" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Acqua,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::Altro { is_punto: false },
                },
            }
        }
        // Controlla le strade
        else if let Some((_, v)) = way_data.tags.iter().find(|(k, _)| k == "highway") {
            match v.as_str() {
                "motorway" | "trunk" | "primary" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::StradaPrincipale,
                },
                "secondary" | "tertiary" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::StradaSecondaria,
                },
                "residential" | "service" | "unclassified" | "living_street" => {
                    MapElement {
                        id: way_data.id,
                        vertices: vertices.clone(),
                        element_type: ElementType::StradaLocale,
                    }
                }
                "footway" | "path" | "cycleway" | "pedestrian" | "steps" => {
                    MapElement {
                        id: way_data.id,
                        vertices: vertices.clone(),
                        element_type: ElementType::StradaPedonale,
                    }
                }
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    element_type: ElementType::StradaLocale,
                },
            }
        }
        // Altro
        else {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                element_type: ElementType::Altro { is_punto: false },
            }
        }
    };

    Some(elemento)
}

/// Risultato della conversione OSM
pub struct ConversionResult {
    /// Elementi della mappa convertiti
    pub elementi: Vec<MapElement>,
    /// ID dei nodi OSM che fanno parte di ways (linee o poligoni)
    /// Questi nodi non dovrebbero essere renderizzati come punti separati
    pub nodi_in_ways: std::collections::HashSet<i64>,
}

/// Converte una collezione di nodi e ways OSM in elementi della mappa
pub fn converti_elementi_osm(
    nodes_data: &[NodeData],
    ways_data: &[WayData],
    nodi_nel_raggio: &HashMap<i64, (f64, f64)>,
) -> ConversionResult {
    use rayon::prelude::*;

    let mut elementi: Vec<MapElement> = Vec::new();

    // Prima passata: raccogli tutti gli ID dei nodi usati dalle ways (in parallelo)
    let nodi_in_ways: std::collections::HashSet<i64> = ways_data
        .par_iter()
        .flat_map(|way_data| {
            way_data.node_refs
                .par_iter()
                .filter(|&&node_id| nodi_nel_raggio.contains_key(&node_id))
                .copied()
        })
        .collect();

    // Converti i nodi in parallelo
    let nodi_elementi: Vec<MapElement> = nodes_data
        .par_iter()
        .filter_map(|node_data| converti_nodo(node_data, nodi_nel_raggio))
        .collect();

    // Converti le ways in parallelo
    let ways_elementi: Vec<MapElement> = ways_data
        .par_iter()
        .filter_map(|way_data| converti_way(way_data, nodi_nel_raggio))
        .collect();

    elementi.extend(nodi_elementi);
    elementi.extend(ways_elementi);

    // Ordina per ID per garantire consistenza
    elementi.sort_by_key(|e| e.id());

    ConversionResult {
        elementi,
        nodi_in_ways,
    }
}

