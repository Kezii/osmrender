use embedded_gfx::{K3dengine, draw::draw};
use embedded_graphics::prelude::OriginDimensions;
use embedded_graphics_core::{
    draw_target::DrawTarget,
    pixelcolor::{Rgb565, RgbColor as _},
};
use image::RgbImage;
use log::error;
use nalgebra::Point3;

use crate::map_elements::MapElement;
use crate::rendering_adapter::{ConversionParams, converti_a_mesh};

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

/// Renderizza la mappa degli elementi ad alto livello nel raggio specificato
pub fn renderizza_mappa<
    D: DrawTarget<Color = embedded_graphics_core::pixelcolor::Rgb565> + OriginDimensions,
>(
    elementi: &[MapElement],
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
    framebuffer: &mut D,
) -> Result<(), <D as DrawTarget>::Error>
where
    <D as DrawTarget>::Error: std::fmt::Debug,
{
    if elementi.is_empty() {
        println!("Nessun elemento da renderizzare");
        return Ok(());
    }

    // Aggiungi un margine per vedere meglio
    let margine = raggio_metri * 0.1; // 10% di margine
    let raggio_con_margine = raggio_metri + margine;

    // Calcola bounds dal centro con margine
    // Approssimazione semplice: 1 grado lat ≈ 111 km, 1 grado lon ≈ 111 km * cos(lat)
    let gradi_lat = raggio_con_margine / 111000.0;
    let gradi_lon = raggio_con_margine / (111000.0 * centro_lat.to_radians().cos());

    // Inizializza i bounds dal centro con margine
    let mut min_lat = centro_lat - gradi_lat;
    let mut max_lat = centro_lat + gradi_lat;
    let mut min_lon = centro_lon - gradi_lon;
    let mut max_lon = centro_lon + gradi_lon;

    // Scala per mantenere le coordinate in un range che la proiezione può gestire
    let scale_factor = 0.0003; // Scala più grande per ingrandire la mappa

    // Crea i parametri di conversione
    // Usiamo z diversi per priorità: priorità più alta = z più alto (più vicino alla camera)
    // Questo assicura che gli edifici (priorità 2) siano sopra le aree (priorità 0-1)
    // e le strade (priorità 3) siano sopra gli edifici
    // z_spacing più grande per garantire che gli edifici siano sempre visibili
    let params = ConversionParams {
        min_lat,
        max_lat,
        min_lon,
        max_lon,
        width: framebuffer.size().width as u32,
        height: framebuffer.size().height as u32,
        scale_factor,
        z_base: 0.0,     // Base z per elementi con priorità 0
        z_spacing: 0.01, // Spaziatura tra i livelli di priorità (più grande per garantire visibilità)
    };

    // Crea l'engine 3D
    let mut engine = K3dengine::new(
        framebuffer.size().width as u16,
        framebuffer.size().height as u16,
    );

    // Configura la camera per vedere gli oggetti a z=0
    // Dopo la trasformazione view, z diventa la distanza dalla camera
    // Se camera è a z=-5 e oggetti a z=0, dopo view gli oggetti sono a z=5
    engine.camera.near = 0.1;
    engine.camera.far = 100.0;

    // Posiziona la camera più vicina per zoomare sulla mappa
    // Distanza più piccola = zoom maggiore
    engine.camera.set_position(Point3::new(0.0, 0.0, 2.0));
    engine.camera.set_target(Point3::new(0.0, 0.0, 0.0));
    // FOV più stretto per zoomare di più (30 gradi invece di 90)
    engine.camera.set_fovy(std::f32::consts::PI / 6.0);

    // Usa rendering_adapter per creare le mesh
    let mesh_container = converti_a_mesh(elementi, params);
    let meshes = mesh_container.get_meshes();

    // Renderizza tutte le mesh
    // L'API si aspetta IntoIterator<Item = &K3dMesh>, quindi passiamo &meshes
    let mut primitive_count = 0;
    engine.render(&meshes, |p| {
        primitive_count += 1;
        let e = draw(&p, framebuffer);

        if let Err(e) = e {
            error!("Error drawing primitive: {:?} {:?}", p, e);
        }
    });
    println!("Renderizzati {} primitivi", primitive_count);

    Ok(())
}
