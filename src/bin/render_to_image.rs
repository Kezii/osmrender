use embedded_graphics::prelude::OriginDimensions;
use image::RgbImage;
use osmrender::{imageframebuffer::ImageFramebuffer, renderprocess::RenderState};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Esempio: coordinate di Milano (puoi modificare queste coordinate)
    let centro_lat = 45.47362;
    let centro_lon = 9.24919;
    let raggio_metri = 1000.0;

    //print_from_id("nord-ovest-251207.osm.pbf", 159322216)?;

    //return Ok(());

    // Dimensioni dell'immagine
    let width = 4000u32;
    let height = 4000u32;

    // Crea il framebuffer con sfondo beige chiaro per un aspetto più naturale
    let mut buffer = vec![0u8; (width * height * 3) as usize];
    for i in (0..buffer.len()).step_by(3) {
        buffer[i] = 245; // R
        buffer[i + 1] = 240; // G
        buffer[i + 2] = 230; // B (beige chiaro)
    }
    let mut framebuffer = ImageFramebuffer {
        width,
        height,
        buffer,
    };

    let mut render_state = RenderState::default();
    render_state.set_bbox_for_viewport(centro_lat, centro_lon, raggio_metri, framebuffer.size());
    render_state.reload_chunks()?;
    render_state.reload_map_elements()?;
    render_state.reload_mesh_container(&mut framebuffer)?;

    // Usa la nuova funzione per stampare solo gli elementi nel raggio
    render_state.renderizza_mappa(&mut framebuffer).unwrap();

    // Converti il framebuffer in RgbImage e salva
    let img = RgbImage::from_raw(width, height, framebuffer.buffer)
        .ok_or("Failed to create image from framebuffer")?;

    let output_path = "mappa.png";

    img.save(output_path)?;
    println!("Mappa salvata in: {}", output_path);

    Ok(())
}
