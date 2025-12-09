use osmpbf::{Element, ElementReader};
use std::collections::HashMap;
use std::sync::Arc;
use rayon::prelude::*;

mod render;
mod map_elements;
mod converter;
mod rendering_adapter;

use converter::{NodeData, WayData, converti_elementi_osm, ConversionResult};


/// Calcola la distanza in metri tra due coordinate geografiche usando la formula di Haversine
fn distanza_geografica(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371000.0; // Raggio della Terra in metri
    
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    
    let a = (d_lat / 2.0).sin().powi(2) +
            lat1.to_radians().cos() * lat2.to_radians().cos() *
            (d_lon / 2.0).sin().powi(2);
    
    let c = 2.0 * a.sqrt().asin();
    
    R * c
}

/// Verifica se un punto è entro un raggio specificato da un punto centrale
fn entro_raggio(lat: f64, lon: f64, centro_lat: f64, centro_lon: f64, raggio_metri: f64) -> bool {
    distanza_geografica(lat, lon, centro_lat, centro_lon) <= raggio_metri
}

// Le strutture NodeData e WayData sono ora in converter.rs

// Struttura per accumulare i risultati durante il map-reduce parallelo
#[derive(Clone)]
struct Accumulator {
    nodes: Vec<NodeData>,
    ways: Vec<WayData>,
    relations: u64,
}

/// Stampa gli elementi OSM solo se sono entro un raggio specificato
fn stampa_elementi_in_raggio(
    percorso_file: &str,
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Fase 1: Lettura e raccolta elementi (in parallelo) ---");
    
    let reader = ElementReader::from_path(percorso_file)?;
    let accumulator = reader.par_map_reduce(
        |element| {
            match element {
                Element::DenseNode(node) => {
                    let tags: Vec<(String, String)> = node.tags()
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
                        relations: 0,
                    }
                }
                Element::Node(_node) => {
                    panic!("Node is not supported");
                }
                Element::Way(way) => {
                    let node_refs: Vec<i64> = way.refs().collect();
                    let tags: Vec<(String, String)> = way.tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    Accumulator {
                        nodes: Vec::new(),
                        ways: vec![WayData {
                            id: way.id(),
                            node_refs,
                            tags,
                        }],
                        relations: 0,
                    }
                }
                Element::Relation(_rel) => {
                    Accumulator {
                        nodes: Vec::new(),
                        ways: Vec::new(),
                        relations: 1,
                    }
                }
            }
        },
        || Accumulator {
            nodes: Vec::new(),
            ways: Vec::new(),
            relations: 0,
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
            relations: a.relations + b.relations,
        },
    )?;
    
    let nodes_data = accumulator.nodes;
    let ways_data = accumulator.ways;
    let relations_count = accumulator.relations;
    
    println!("Letti {} nodi, {} ways, {} relazioni", nodes_data.len(), ways_data.len(), relations_count);
    
    // Primo passaggio parallelo: identifica i nodi nel raggio
    println!("--- Fase 2: Analisi nodi nel raggio (in parallelo) ---");
    let nodi_nel_raggio: HashMap<i64, (f64, f64)> = nodes_data
        .par_iter()
        .filter(|node_data| entro_raggio(node_data.lat, node_data.lon, centro_lat, centro_lon, raggio_metri))
        .map(|node_data| (node_data.id, (node_data.lat, node_data.lon)))
        .collect();
    
    println!("Trovati {} nodi nel raggio di {:.0} metri\n", nodi_nel_raggio.len(), raggio_metri);
    
    // Condividi la HashMap tra i thread usando Arc
    let nodi_nel_raggio = Arc::new(nodi_nel_raggio);
    
    // Fase 3: Conversione da elementi OSM a elementi della mappa ad alto livello
    println!("--- Fase 3: Conversione elementi OSM in elementi mappa (in parallelo) ---");
    println!("Centro: lat {:.6}, lon {:.6}", centro_lat, centro_lon);
    println!("Raggio: {:.0} metri\n", raggio_metri);
    
    // Converti nodi e ways in elementi della mappa ad alto livello
    let ConversionResult { elementi: elementi_mappa, nodi_in_ways } = converti_elementi_osm(
        &nodes_data,
        &ways_data,
        &nodi_nel_raggio,
    );
    
    // Stampa informazioni sugli elementi convertiti
    let mut contatori: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for elemento in &elementi_mappa {
        let tipo = match elemento.element_type {
            map_elements::ElementType::Edificio => "Edificio",
            map_elements::ElementType::StradaPrincipale => "Strada Principale",
            map_elements::ElementType::StradaSecondaria => "Strada Secondaria",
            map_elements::ElementType::StradaLocale => "Strada Locale",
            map_elements::ElementType::StradaPedonale => "Strada Pedonale",
            map_elements::ElementType::Ferrovia => "Ferrovia",
            map_elements::ElementType::Fiume => "Fiume",
            map_elements::ElementType::Canale => "Canale",
            map_elements::ElementType::Parco => "Parco",
            map_elements::ElementType::Acqua => "Acqua",
            map_elements::ElementType::Foresta => "Foresta",
            map_elements::ElementType::Boscaglia => "Boscaglia",
            map_elements::ElementType::Residenziale => "Residenziale",
            map_elements::ElementType::Commerciale => "Commerciale",
            map_elements::ElementType::Industriale => "Industriale",
            map_elements::ElementType::Agricolo => "Agricolo",
            map_elements::ElementType::Aeroporto => "Aeroporto",
            map_elements::ElementType::Cimitero => "Cimitero",
            map_elements::ElementType::CampoSportivo => "Campo Sportivo",
            map_elements::ElementType::Albero => "Albero",
            map_elements::ElementType::PuntoInteresse { .. } => "Punto Interesse",
            map_elements::ElementType::Altro { .. } => "Altro",
        };
        *contatori.entry(tipo.to_string()).or_insert(0) += 1;
    }
    
    println!("\n--- Riepilogo Elementi Mappa ---");
    for (tipo, count) in &contatori {
        println!("{}: {}", tipo, count);
    }
    println!("\nTotale elementi da renderizzare: {}", elementi_mappa.len());
    println!("Relazioni processate: {}", relations_count);

    // Renderizza la mappa usando gli elementi ad alto livello
    println!("\n--- Rendering mappa ---");
    render::renderizza_mappa(
        &elementi_mappa,
        &nodi_in_ways,
        centro_lat,
        centro_lon,
        raggio_metri,
        "mappa.png",
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Esempio: coordinate di Milano (puoi modificare queste coordinate)
    let centro_lat = 45.48244707211893;
    let centro_lon = 9.23972904485661;
    let raggio_metri = 500.0;

    // Usa la nuova funzione per stampare solo gli elementi nel raggio
    stampa_elementi_in_raggio("nord-ovest-251207.osm.pbf", centro_lat, centro_lon, raggio_metri)?;

    Ok(())
}
