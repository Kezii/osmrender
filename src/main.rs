use osmrender::chunk_manager::{ChunkConfig, GeoBBox, load_chunks_for_bbox};
use osmrender::map_elements::{ElementType, MapElement};
use osmrender::raw_osm_reader::{RawOsmData, RelationMemberType};
use std::collections::HashSet;
use std::time::Instant;

use osmrender::render;

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

// Le strutture NodeData e WayData sono ora in converter.rs

/// Stampa gli elementi OSM solo se sono entro un raggio specificato
fn stampa_elementi_in_raggio(
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    //let mut spatial = load_primitives_in_bbox("chunks", bbox, ChunkConfig { chunk_size_m: 10000.0 })?;
    /*
        let accumulator = read_raw_osm_file("nord-ovest-251207.osm.pbf")?;


        // Primo passaggio parallelo: identifica i nodi nel raggio
        println!("--- Fase 2: Analisi nodi nel raggio (in parallelo) ---");

        //let accumulator = filtra_raw_osm_data(accumulator, centro_lat, centro_lon, raggio_metri);

        println!(
            "Trovati {} nodi nel raggio di {:.0} metri\n",
            accumulator.nodes.len(),
            raggio_metri
        );

        // Fase 3: Conversione da elementi OSM a elementi della mappa ad alto livello
        println!("--- Fase 3: Conversione elementi OSM in elementi mappa (in parallelo) ---");
        println!("Centro: lat {:.6}, lon {:.6}", centro_lat, centro_lon);
        println!("Raggio: {:.0} metri\n", raggio_metri);

        // Converti nodi, ways e relazioni in elementi della mappa ad alto livello
        let elementi_mappa = converti_elementi_osm_posizionati(accumulator);

    */

    let now = Instant::now();
    let bbox = GeoBBox {
        min_lat: centro_lat - raggio_metri / 111000.0,
        max_lat: centro_lat + raggio_metri / 111000.0,
        min_lon: centro_lon - raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
        max_lon: centro_lon + raggio_metri / (111000.0 * centro_lat.to_radians().cos()),
    };

    //let mut spatial = load_primitives_in_bbox("chunks", bbox, ChunkConfig { chunk_size_m: 10000.0 })?;
    let cfg = ChunkConfig {
        chunk_size_m: 20000.0,
    };
    let chunks = load_chunks_for_bbox::<MapElement>("chunks", &bbox, cfg)?;

    let elapsed = now.elapsed();
    println!("Tempo di caricamento chunk: {:?}", elapsed);

    let chunk_bboxes = chunks.iter().map(|c| c.bbox()).collect::<Vec<_>>();
    let elementi_mappa = chunks
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

    // Renderizza la mappa usando gli elementi ad alto livello
    render::renderizza_mappa(
        &elementi_mappa,
        centro_lat,
        centro_lon,
        raggio_metri,
        "mappa.png",
    )?;

    let elapsed = now.elapsed();
    println!("Tempo di rendering mappa: {:?}", elapsed);
    Ok(())
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    // Esempio: coordinate di Milano (puoi modificare queste coordinate)
    let centro_lat = 45.47362;
    let centro_lon = 9.24919;
    let raggio_metri = 10500.0;

    //print_from_id("nord-ovest-251207.osm.pbf", 159322216)?;

    //return Ok(());

    // Usa la nuova funzione per stampare solo gli elementi nel raggio
    stampa_elementi_in_raggio(centro_lat, centro_lon, raggio_metri)?;

    Ok(())
}
