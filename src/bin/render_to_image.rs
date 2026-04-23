use embedded_gfx::framebuffer::StackFramebuffer;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, OriginDimensions, Point, RgbColor},
    primitives::Rectangle,
};
use image::RgbImage;
use osmrender::{
    GeoPos,
    chunk_manager::{ChunkConfig, ChunkManager, StdFsChunkStorage},
    imageframebuffer::ImageFramebuffer,
    renderprocess::RenderState,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Esempio: coordinate di Milano (puoi modificare queste coordinate)
    let centro = GeoPos::new(45.47362, 9.24919);

    //print_from_id("nord-ovest-251207.osm.pbf", 159322216)?;

    //return Ok(());

    // Dimensioni dell'immagine
    const WIDTH: usize = 1000;
    const HEIGHT: usize = 1000;

    // Crea il framebuffer con sfondo beige chiaro per un aspetto più naturale
    let mut buffer = vec![0u8; WIDTH * HEIGHT * 3];
    for i in (0..buffer.len()).step_by(3) {
        buffer[i] = 245; // R
        buffer[i + 1] = 240; // G
        buffer[i + 2] = 230; // B (beige chiaro)
    }
    let mut framebuffer = ImageFramebuffer {
        width: WIDTH as u32,
        height: HEIGHT as u32,
        buffer,
    };

    let mut stackframebuffer = StackFramebuffer::<WIDTH, HEIGHT, Rgb565>::new(Rgb565::BLACK);

    let chunk_store = StdFsChunkStorage::new("chunks");
    let chunk_manager = ChunkManager::new(
        chunk_store,
        ChunkConfig {
            chunk_size_m: 2000.0,
        },
    );
    let mut render_state = RenderState::new(chunk_manager, centro, framebuffer.size());
    render_state.map_to_mesh(centro)?;
    render_state
        .renderizza_mappa(&mut stackframebuffer)
        .unwrap();

    let buffer = stackframebuffer.framebuffer.iter().flatten();

    let area = Rectangle::new(Point::new(0, 0), stackframebuffer.size());

    framebuffer.fill_contiguous(&area, buffer.copied()).unwrap();

    // Converti il framebuffer in RgbImage e salva
    let img = RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, framebuffer.buffer)
        .ok_or("Failed to create image from framebuffer")?;

    let output_path = "mappa.png";

    img.save(output_path)?;
    println!("Mappa salvata in: {}", output_path);

    Ok(())
}
