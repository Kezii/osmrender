use osmpbf::{Element, ElementReader};

use osmrender::chunk_manager::{ChunkConfig, ChunkPrimitive, save_chunks};
use osmrender::converter::converti_elementi_osm_posizionati;
use osmrender::map_elements::MapElement;
use osmrender::raw_osm_reader::read_raw_osm_file;
use osmrender::spatial_index::{PositionedPrimitive, build_spatial_index};


fn main() -> Result<(), Box<dyn std::error::Error>> {

    let input = "nord-ovest-251207.osm.pbf";
    let out_dir = "chunks";
    let cfg = ChunkConfig { chunk_size_m: 20000.0 };

    println!("Input: {}", input);
    println!("Output dir: {}", out_dir);
    println!("Chunk size (m): {}", cfg.chunk_size_m);

    println!("--- Fase 1: Lettura e raccolta elementi (in parallelo) ---");

    let accumulator = read_raw_osm_file(input)?;


    // Rappresentazione intermedia: primitive OSM indicizzate per posizione (griglia lat/lon)
    // Nota: per ways/relations la posizione è una "rappresentativa" (centro bbox dei nodi).
    println!("--- Fase 1b: Indicizzazione spaziale primitive OSM ---");

    let elementi_mappa = converti_elementi_osm_posizionati(accumulator);

    //let spatial = build_spatial_index(accumulator.nodes, accumulator.ways, accumulator.relations);
    //println!("Primitive posizionate: {}", spatial.len());

    println!("--- Salvataggio chunk ---");
    let spatial = elementi_mappa.iter().map(|e| ChunkPrimitive { bbox: e.bbox(), primitive: e.clone() }).collect::<Vec<_>>();
    save_chunks::<MapElement>(&spatial, out_dir, cfg)?;
    println!("Done.");

    Ok(())
}


