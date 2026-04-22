use osmrender::chunk_manager::{ChunkConfig, StdFsChunkStorage, save_chunks};
use osmrender::converter::converti_elementi_osm_posizionati;
use osmrender::map_elements::MapElement;
use osmrender::raw_osm_reader::read_raw_osm_file;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let input = "nord-ovest-251207.osm.pbf";
    let out_dir = "chunks";
    let cfg = ChunkConfig {
        chunk_size_m: 2000.0,
    };

    println!("Input: {}", input);
    println!("Output dir: {}", out_dir);
    println!("Chunk size (m): {}", cfg.chunk_size_m);

    println!("--- Fase 1: Lettura e raccolta elementi (in parallelo) ---");

    let raw_osm = read_raw_osm_file(input)?;

    // Rappresentazione intermedia: primitive OSM indicizzate per posizione (griglia lat/lon)
    // Nota: per ways/relations la posizione è una "rappresentativa" (centro bbox dei nodi).
    println!("--- Fase 1b: Indicizzazione spaziale primitive OSM ---");

    let mut elementi_mappa = converti_elementi_osm_posizionati(raw_osm);

    elementi_mappa.sort_by_key(|e| e.id);
    //let spatial = build_spatial_index(accumulator.nodes, accumulator.ways, accumulator.relations);
    //println!("Primitive posizionate: {}", spatial.len());

    println!("--- Salvataggio chunk ---");

    let chunk_store = StdFsChunkStorage::new(out_dir);

    save_chunks(elementi_mappa, &chunk_store, cfg)?;
    println!("Done.");

    Ok(())
}
