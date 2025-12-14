use osmpbf::{Element, ElementReader};
use osmrender::chunk_manager::{ChunkConfig, GeoBBox, load_primitives_in_bbox};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

use osmrender::converter::{
    ConversionResult, NodeData, RelationData, RelationMember, RelationMemberType, WayData,
    converti_elementi_osm_posizionati,
};
use osmrender::spatial_index::{OsmPrimitive, PositionedPrimitive, build_spatial_index};
use osmrender::{map_elements, render};

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

// Le strutture NodeData e WayData sono ora in converter.rs

// Struttura per accumulare i risultati durante il map-reduce parallelo
#[derive(Clone)]
struct Accumulator {
    nodes: Vec<NodeData>,
    ways: Vec<WayData>,
    relations: Vec<RelationData>,
}

/// Stampa gli elementi OSM solo se sono entro un raggio specificato
fn stampa_elementi_in_raggio(
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
) -> Result<(), Box<dyn std::error::Error>> {

    let bbox = GeoBBox {
        min_lat: centro_lat - raggio_metri / 111000.0,
        max_lat: centro_lat + raggio_metri / 111000.0,
        min_lon: centro_lon - raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
        max_lon: centro_lon + raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
    };

    let spatial = load_primitives_in_bbox("chunks", bbox, ChunkConfig { chunk_size_m: 10000.0 })?;
    
    // Primo passaggio parallelo: identifica i nodi nel raggio
    println!("--- Fase 2: Analisi nodi nel raggio (in parallelo) ---");
    let nodi_nel_raggio: HashMap<i64, (f64, f64)> = spatial
        .par_iter()
        .filter_map(|p| match &p.primitive {
            OsmPrimitive::Node(n) => {
                if entro_raggio(p.lat, p.lon, centro_lat, centro_lon, raggio_metri) {
                    Some((n.id, (p.lat, p.lon)))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    println!(
        "Trovati {} nodi nel raggio di {:.0} metri\n",
        nodi_nel_raggio.len(),
        raggio_metri
    );

    // Condividi la HashMap tra i thread usando Arc
    let nodi_nel_raggio = Arc::new(nodi_nel_raggio);

    // Fase 3: Conversione da elementi OSM a elementi della mappa ad alto livello
    println!("--- Fase 3: Conversione elementi OSM in elementi mappa (in parallelo) ---");
    println!("Centro: lat {:.6}, lon {:.6}", centro_lat, centro_lon);
    println!("Raggio: {:.0} metri\n", raggio_metri);

    // Converti nodi, ways e relazioni in elementi della mappa ad alto livello
    let ConversionResult {
        elementi: elementi_mappa,
        nodi_in_ways,
    } = converti_elementi_osm_posizionati(&spatial, &nodi_nel_raggio);


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


#[allow(dead_code)]
fn print_from_id(percorso_file: &str, id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let reader = ElementReader::from_path(percorso_file)?;
    
    reader.for_each(|element|
    {
        let element_id = match &element {
            Element::DenseNode(node) => node.id(),
            Element::Node(node) => node.id(),
            Element::Way(way) => way.id(),
            Element::Relation(rel) => rel.id(),
        };
        if element_id == id {
            match &element {
                Element::DenseNode(node) => println!("{:#?}", node.info()),
                Element::Node(node) => println!("{:#?}", node.info()),
                Element::Way(way) => println!("{:#?}", way.info()),
                Element::Relation(rel) => println!("{:#?}", rel.info()),
            }
        }
    })?;


    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    // Esempio: coordinate di Milano (puoi modificare queste coordinate)
    let centro_lat = 45.47362;
    let centro_lon = 9.24919;
    let raggio_metri = 3500.0;

    //print_from_id("nord-ovest-251207.osm.pbf", 159322216)?;

    //return Ok(());

    // Usa la nuova funzione per stampare solo gli elementi nel raggio
    stampa_elementi_in_raggio(
        centro_lat,
        centro_lon,
        raggio_metri,
    )?;

    Ok(())
}
