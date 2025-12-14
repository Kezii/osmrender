use log::debug;
use serde::{Deserialize, Serialize};

use crate::{map_elements::{ElementType, MapElement}, raw_osm_reader::{NodeData, RawOsmData, RelationData, RelationMemberType, WayData}};
use std::collections::HashMap;
// use crate::spatial_index::{OsmPrimitive, PositionedPrimitive};


/// nell'array spatial non serve ripetere lat e lon
#[derive(Clone, Debug, Serialize, Deserialize,bincode::Encode, bincode::Decode)]
pub struct SpatialNodeData {
    pub id: i64,
    pub tags: Vec<(String, String)>,
}

/// Converte un nodo OSM in un elemento della mappa
pub fn converti_nodo(
    node_data: &NodeData,
) -> Option<MapElement> {

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
                    inner_rings: Vec::new(),
                    element_type: ElementType::Albero,
                }
            } else {
                MapElement {
                    id: node_data.id,
                    vertices: vec![(node_data.lat, node_data.lon)],
                    inner_rings: Vec::new(),
                    element_type: ElementType::Altro { is_punto: true },
                }
            }
        }
        // Controlla se è un punto di interesse
        else if name.is_some()
            || node_data.tags.iter().any(|(k, _)| {
                matches!(
                    k.as_str(),
                    "place" | "amenity" | "shop" | "tourism" | "leisure" | "historic"
                )
            })
        {
            MapElement {
                id: node_data.id,
                vertices: vec![(node_data.lat, node_data.lon)],
                inner_rings: Vec::new(),
                element_type: ElementType::PuntoInteresse {
                    ha_nome: name.is_some(),
                },
            }
        } else {
            MapElement {
                id: node_data.id,
                vertices: vec![(node_data.lat, node_data.lon)],
                inner_rings: Vec::new(),
                element_type: ElementType::Altro { is_punto: true },
            }
        }
    };

    Some(elemento)
}

/// Converte una way OSM in un elemento della mappa
pub fn converti_way(
    way_data: &WayData,
    node_index: &HashMap<i64, (f64, f64)>,
) -> Option<MapElement> {
    // Raccogli i nodi della way che sono nel raggio
    let vertices: Vec<(f64, f64)> = way_data
        .node_refs
        .iter()
        .filter_map(|&node_id| node_index.get(&node_id).copied())
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
                    inner_rings: Vec::new(),
                    element_type: ElementType::Fiume,
                },
                "canal" | "ditch" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Canale,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Altro { is_punto: false },
                },
            }
        }
        // Controlla le ferrovie
        else if way_data.tags.iter().any(|(k, _)| k == "railway") {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                inner_rings: Vec::new(),
                element_type: ElementType::Ferrovia,
            }
        }
        // Controlla gli edifici
        else if way_data.tags.iter().any(|(k, _)| k == "building") {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                inner_rings: Vec::new(),
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
                inner_rings: Vec::new(),
                element_type: ElementType::Aeroporto,
            }
        }
        // Controlla landuse e leisure (residenziale, commerciale, industriale, agricolo, cimitero, parchi)
        else if let Some((k, v)) = way_data
            .tags
            .iter()
            .find(|(k, _)| k == "landuse" || k == "leisure")
        {
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
                    inner_rings: Vec::new(),
                    element_type: ElementType::Parco,
                },
                // Altri landuse
                ("landuse", "residential") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Residenziale,
                },
                ("landuse", "commercial") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Commerciale,
                },
                ("landuse", "industrial") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Industriale,
                },
                ("landuse", "farmland") | ("landuse", "farmyard") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Agricolo,
                },
                ("landuse", "cemetery") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Cimitero,
                },
                // Acqua da landuse
                ("landuse", "basin") | ("landuse", "reservoir") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Acqua,
                },
                // Foreste da landuse
                ("landuse", "forest") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Foresta,
                },
                // Campi sportivi
                ("leisure", "pitch") => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::CampoSportivo,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
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
                    inner_rings: Vec::new(),
                    element_type: ElementType::Acqua,
                },
                "wood" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Foresta,
                },
                "scrub" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::Boscaglia,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
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
                    inner_rings: Vec::new(),
                    element_type: ElementType::Acqua,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
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
                    inner_rings: Vec::new(),
                    element_type: ElementType::StradaPrincipale,
                },
                "secondary" | "tertiary" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::StradaSecondaria,
                },
                "residential" | "service" | "unclassified" | "living_street" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::StradaLocale,
                },
                "footway" | "path" | "cycleway" | "pedestrian" | "steps" => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::StradaPedonale,
                },
                _ => MapElement {
                    id: way_data.id,
                    vertices: vertices.clone(),
                    inner_rings: Vec::new(),
                    element_type: ElementType::StradaLocale,
                },
            }
        }
        // Altro
        else {
            MapElement {
                id: way_data.id,
                vertices: vertices.clone(),
                inner_rings: Vec::new(),
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

/// Converte una relazione multipolygon in un elemento della mappa
fn converti_multipolygon(
    relation_data: &RelationData,
    ways_data: &HashMap<i64, &WayData>,
    node_index: &HashMap<i64, (f64, f64)>,
) -> Option<MapElement> {
    // Raccogli gli anelli outer e inner
    let mut outer_ways: Vec<&WayData> = Vec::new();
    let mut inner_ways: Vec<&WayData> = Vec::new();

    for member in &relation_data.members {
        if member.member_type != RelationMemberType::Way {
            continue;
        }

        if let Some(way) = ways_data.get(&member.member_id) {
            match member.role.as_str() {
                "outer" => outer_ways.push(way),
                "inner" => inner_ways.push(way),
                _ => {} // Ignora altri ruoli
            }
        }
    }

    if outer_ways.is_empty() {
        return None;
    }

    // Costruisci gli anelli outer combinando le ways
    // Le ways possono essere chiuse (primo = ultimo nodo) o aperte che devono essere collegate
    #[allow(clippy::type_complexity)]
    fn combina_ways_in_ring(
        ways: &[&WayData],
        node_index: &HashMap<i64, (f64, f64)>,
    ) -> Option<Vec<(f64, f64)>> {
        if ways.is_empty() {
            return None;
        }

        // Costruisci una lista di (node_ids, vertices) per ogni way
        let mut way_data: Vec<(Vec<i64>, Vec<(f64, f64)>)> = Vec::new();
        for way in ways {
            let mut node_ids = Vec::new();
            let mut vertices = Vec::new();
            for &node_id in &way.node_refs {
                if let Some(&coord) = node_index.get(&node_id) {
                    node_ids.push(node_id);
                    vertices.push(coord);
                }
            }
            if node_ids.len() >= 2 {
                way_data.push((node_ids, vertices));
            }
        }

        if way_data.is_empty() {
            return None;
        }

        // Se c'è una sola way, usala direttamente
        if way_data.len() == 1 {
            let (_, vertices) = &way_data[0];
            if vertices.len() >= 3 {
                let mut result = vertices.clone();
                // Chiudi l'anello se non è già chiuso
                if result[0] != result[result.len() - 1] {
                    result.push(result[0]);
                }
                return Some(result);
            }
            return None;
        }

        // Combina le ways trovando i punti di connessione usando i node_id
        let mut used_ways = std::collections::HashSet::new();
        let mut ring_node_ids: Vec<i64> = Vec::new();
        let mut ring_vertices: Vec<(f64, f64)> = Vec::new();

        // Inizia con la prima way
        if let Some((first_node_ids, first_vertices)) = way_data.first() {
            ring_node_ids.extend(first_node_ids.iter().copied());
            ring_vertices.extend(first_vertices.iter().copied());
            used_ways.insert(0);

            // Continua a collegare ways finché possibile
            let mut changed = true;
            while changed && used_ways.len() < way_data.len() {
                changed = false;

                for (way_idx, (node_ids, vertices)) in way_data.iter().enumerate() {
                    if used_ways.contains(&way_idx) {
                        continue;
                    }

                    let ring_start_id = ring_node_ids[0];
                    let ring_end_id = ring_node_ids[ring_node_ids.len() - 1];
                    let way_start_id = node_ids[0];
                    let way_end_id = node_ids[node_ids.len() - 1];

                    // Collega se: way_end == ring_start (aggiungi all'inizio, invertita)
                    if way_end_id == ring_start_id {
                        let mut node_ids_copy = node_ids.clone();
                        let mut vertices_copy = vertices.clone();
                        node_ids_copy.pop(); // Rimuovi l'ultimo (duplicato)
                        vertices_copy.pop();
                        node_ids_copy.reverse();
                        vertices_copy.reverse();
                        ring_node_ids = node_ids_copy
                            .into_iter()
                            .chain(ring_node_ids.into_iter().skip(1))
                            .collect();
                        ring_vertices = vertices_copy
                            .into_iter()
                            .chain(ring_vertices.into_iter().skip(1))
                            .collect();
                        used_ways.insert(way_idx);
                        changed = true;
                        break;
                    }
                    // Collega se: way_start == ring_end (aggiungi alla fine)
                    else if way_start_id == ring_end_id {
                        let mut node_ids_copy = node_ids.clone();
                        let mut vertices_copy = vertices.clone();
                        node_ids_copy.remove(0); // Rimuovi il primo (duplicato)
                        vertices_copy.remove(0);
                        ring_node_ids.extend(node_ids_copy);
                        ring_vertices.extend(vertices_copy);
                        used_ways.insert(way_idx);
                        changed = true;
                        break;
                    }
                    // Collega se: way_end == ring_end (aggiungi alla fine, invertita)
                    else if way_end_id == ring_end_id {
                        let mut node_ids_copy = node_ids.clone();
                        let mut vertices_copy = vertices.clone();
                        node_ids_copy.pop(); // Rimuovi l'ultimo (duplicato)
                        vertices_copy.pop();
                        node_ids_copy.reverse();
                        vertices_copy.reverse();
                        ring_node_ids.extend(node_ids_copy);
                        ring_vertices.extend(vertices_copy);
                        used_ways.insert(way_idx);
                        changed = true;
                        break;
                    }
                    // Collega se: way_start == ring_start (aggiungi all'inizio)
                    else if way_start_id == ring_start_id {
                        let mut node_ids_copy = node_ids.clone();
                        let mut vertices_copy = vertices.clone();
                        node_ids_copy.remove(0); // Rimuovi il primo (duplicato)
                        vertices_copy.remove(0);
                        node_ids_copy.reverse();
                        vertices_copy.reverse();
                        ring_node_ids = node_ids_copy
                            .into_iter()
                            .chain(ring_node_ids.into_iter().skip(1))
                            .collect();
                        ring_vertices = vertices_copy
                            .into_iter()
                            .chain(ring_vertices.into_iter().skip(1))
                            .collect();
                        used_ways.insert(way_idx);
                        changed = true;
                        break;
                    }
                }
            }

            // Verifica se tutte le ways sono state collegate
            if used_ways.len() < way_data.len() {
                debug!(
                    "⚠️  combina_ways_in_ring: non tutte le ways sono state collegate ({}/{})",
                    used_ways.len(),
                    way_data.len()
                );
                debug!(
                    "   Ways non collegate potrebbero essere condivise con altri multipolygon o avere nodi mancanti"
                );
                // Continua comunque con le ways collegate
            }
        }

        if ring_vertices.len() >= 3 {
            // Verifica se l'anello è chiuso, altrimenti chiudilo
            if ring_node_ids[0] != ring_node_ids[ring_node_ids.len() - 1] {
                // Se l'anello non è chiuso, potrebbe essere un problema con ways condivise
                // Prova a chiudere solo se i vertici sono abbastanza vicini
                let first_vertex = ring_vertices[0];
                let last_vertex = ring_vertices[ring_vertices.len() - 1];
                let dist = ((first_vertex.0 - last_vertex.0).powi(2)
                    + (first_vertex.1 - last_vertex.1).powi(2))
                .sqrt();
                if dist < 0.0001 {
                    // Soglia molto piccola per coordinate geografiche
                    ring_vertices.push(ring_vertices[0]);
                } else {
                    debug!(
                        "⚠️  combina_ways_in_ring: anello non chiuso, distanza tra primo e ultimo: {}",
                        dist
                    );
                    // Chiudi comunque per evitare errori di rendering
                    ring_vertices.push(ring_vertices[0]);
                }
            }
            Some(ring_vertices)
        } else {
            None
        }
    }

    let outer_vertices = match combina_ways_in_ring(&outer_ways, node_index) {
        Some(vertices) => vertices,
        None => {
            debug!(
                "⚠️  Impossibile combinare ways outer per multipolygon ID {}",
                relation_data.id
            );
            return None;
        }
    };

    if outer_vertices.len() < 3 {
        return None;
    }

    // Costruisci gli anelli inner (buchi)
    let inner_rings: Vec<Vec<(f64, f64)>> = inner_ways
        .iter()
        .filter_map(|way| {
            let vertices: Vec<(f64, f64)> = way
                .node_refs
                .iter()
                .filter_map(|&node_id| node_index.get(&node_id).copied())
                .collect();
            if vertices.len() >= 3 {
                Some(vertices)
            } else {
                None
            }
        })
        .collect();

    // Determina il tipo di elemento in base ai tag della relazione
    let element_type = {
        if relation_data.tags.iter().any(|(k, _)| k == "building") {
            ElementType::Edificio
        } else if relation_data.tags.iter().any(|(k, _)| k == "leisure") {
            if relation_data
                .tags
                .iter()
                .any(|(k, v)| k == "leisure" && v == "park")
            {
                ElementType::Parco
            } else {
                ElementType::Altro { is_punto: false }
            }
        } else if relation_data.tags.iter().any(|(k, _)| k == "landuse") {
            match relation_data.tags.iter().find(|(k, _)| k == "landuse") {
                Some((_, v)) if v == "residential" => ElementType::Residenziale,
                Some((_, v)) if v == "commercial" => ElementType::Commerciale,
                Some((_, v)) if v == "industrial" => ElementType::Industriale,
                Some((_, v)) if v == "farmland" || v == "agricultural" => ElementType::Agricolo,
                _ => ElementType::Altro { is_punto: false },
            }
        } else if relation_data.tags.iter().any(|(k, _)| k == "natural") {
            match relation_data.tags.iter().find(|(k, _)| k == "natural") {
                Some((_, v)) if v == "water" => ElementType::Acqua,
                Some((_, v)) if v == "wood" || v == "forest" => ElementType::Foresta,
                _ => ElementType::Altro { is_punto: false },
            }
        } else {
            ElementType::Altro { is_punto: false }
        }
    };

    // Crea un MapElement con i buchi
    Some(MapElement {
        id: relation_data.id,
        vertices: outer_vertices,
        inner_rings,
        element_type,
    })
}

/// Converte una collezione di primitive OSM già "posizionate" in elementi della mappa.
/// Versione memory-friendly: non richiede i 3 array separati (nodes/ways/relations).
pub fn converti_elementi_osm_posizionati(
    accumulator: RawOsmData,
) -> Vec<MapElement> {
    use rayon::prelude::*;

    let mut elementi: Vec<MapElement> = Vec::new();

    // Indice rapido id_nodo -> (lat, lon). A questo punto `accumulator.nodes` è già filtrato.
    let node_index: HashMap<i64, (f64, f64)> = accumulator
        .nodes
        .iter()
        .map(|n| (n.id, (n.lat, n.lon)))
        .collect();

    // Mappa ways per accesso rapido (senza clone)
    let ways_map: HashMap<i64, &WayData> = accumulator.ways
        .iter()
        .map(|w| (w.id, w))
        .collect();
    let ways_map = std::sync::Arc::new(ways_map);

    // Prima passata: raccogli tutti gli ID dei nodi usati dalle ways
    let nodi_in_ways: std::collections::HashSet<i64> = ways_map
        .values()
        .flat_map(|w| {
            w.node_refs
                .iter().copied()
        })
        .collect();

    // Converti i nodi in parallelo
    let nodi_elementi: Vec<MapElement> = accumulator.nodes
        .par_iter()
        // Non generare punti per nodi che sono già parte di una way (saranno resi come linea/poligono)
        // Nota: questo rispecchia il comportamento in `rendering_adapter::converti_a_mesh` che li skippa.
        .filter(|n| !nodi_in_ways.contains(&n.id))
        .filter_map(converti_nodo)
        .collect();

    // Converti le ways in parallelo
    let ways_elementi: Vec<MapElement> = accumulator.ways
        .par_iter()
        .filter_map(|w| converti_way(w, &node_index))
        .collect();

    // Converti le relazioni multipolygon in parallelo
    let relations_elementi: Vec<MapElement> = accumulator.relations
        .par_iter()
        .filter_map(|r| converti_multipolygon(r, &ways_map, &node_index))
        .collect();

    elementi.extend(nodi_elementi);
    elementi.extend(ways_elementi);
    elementi.extend(relations_elementi);

    // Ordina per ID per garantire consistenza
    //elementi.sort_by_key(|e| e.id());


    elementi
    
}
