use embedded_gfx::{
    draw::draw,
    mesh::{Geometry, K3dMesh, RenderMode},
    K3dengine,
};
use embedded_graphics_core::{
    draw_target::DrawTarget,
    pixelcolor::{Rgb565, RgbColor as _},
};
use image::RgbImage;
use nalgebra::Point3;

use crate::map_elements::{MapElement, ElementType};

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

/// Framebuffer compatibile con embedded-graphics che usa un buffer RGB888
struct ImageFramebuffer {
    width: u32,
    height: u32,
    buffer: Vec<u8>, // RGB interleaved (Rgb888)
}

impl DrawTarget for ImageFramebuffer {
    type Color = Rgb565;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as u32;
            let y = pixel.0.y as u32;
            
            if x < self.width && y < self.height {
                let idx = ((y * self.width + x) * 3) as usize;
                if idx + 2 < self.buffer.len() {
                    let color = pixel.1;
                    // Converti Rgb565 a Rgb888
                    let r = (color.r() as u16 * 255 / 31) as u8;
                    let g = (color.g() as u16 * 255 / 63) as u8;
                    let b = (color.b() as u16 * 255 / 31) as u8;
                    self.buffer[idx] = r;
                    self.buffer[idx + 1] = g;
                    self.buffer[idx + 2] = b;
                }
            }
        }
        Ok(())
    }
}

impl embedded_graphics_core::geometry::OriginDimensions for ImageFramebuffer {
    fn size(&self) -> embedded_graphics_core::geometry::Size {
        embedded_graphics_core::geometry::Size::new(self.width, self.height)
    }
}

/// Triangola un poligono usando fan triangulation (funziona per poligoni convessi)
fn triangola_poligono(vertices: &[[f32; 3]]) -> Vec<[usize; 3]> {
    if vertices.len() < 3 {
        return Vec::new();
    }
    
    let mut faces = Vec::new();
    // Fan triangulation: ogni triangolo usa il primo vertice e due vertici consecutivi
    for i in 1..vertices.len() - 1 {
        faces.push([0, i, i + 1]);
    }
    faces
}


/// Renderizza la mappa degli elementi ad alto livello nel raggio specificato
pub fn renderizza_mappa(
    elementi: &[MapElement],
    nodi_in_ways: &std::collections::HashSet<i64>,
    centro_lat: f64,
    centro_lon: f64,
    raggio_metri: f64,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

    // Aggiorna bounds con gli elementi effettivi, ma solo per i punti nel perimetro
    for elemento in elementi {
        let coordinate = elemento.coordinate();
        for (lat, lon) in coordinate {
            // Filtra solo i punti nel perimetro per il calcolo dei bounds
            if entro_raggio(lat, lon, centro_lat, centro_lon, raggio_metri) {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
        }
    }

    // Dimensioni dell'immagine
    let width = 4000u32;
    let height = 4000u32;

    // Funzione per convertire coordinate geografiche a coordinate 3D world space
    // Le coordinate devono essere centrate intorno a 0 e scalate per essere visibili
    // Dopo la trasformazione view+projection, z deve essere tra near e far
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    // Scala per mantenere le coordinate in un range che la proiezione può gestire
    // La proiezione perspective converte da world space a clip space
    // Usiamo una scala che mantiene le coordinate nel frustum visibile
    // Usa una scala più grande per ingrandire la mappa
    // Invece di normalizzare a [-1, 1], usiamo un range più grande
    // per riempire meglio lo schermo
    let scale_factor = 0.0003; // Scala più grande per ingrandire la mappa
    
    let to_3d = |lat: f64, lon: f64| -> [f32; 3] {
        let x = ((lon - min_lon) / (max_lon - min_lon) * width as f64) as f32;
        let y = ((lat - min_lat) / (max_lat - min_lat) * height as f64) as f32;
        // Centra le coordinate: da [0, width/height] a [-width/2, width/2]
        // Poi scala per essere nel range visibile dalla camera
        let x_world = (x - center_x) * scale_factor;
        let y_world = (y - center_y) * scale_factor;
        [x_world, y_world, 0.0] // z=0 per rendering 2D
    };

    // Crea il framebuffer con sfondo beige chiaro per un aspetto più naturale
    let mut buffer = vec![0u8; (width * height * 3) as usize];
    for i in (0..buffer.len()).step_by(3) {
        buffer[i] = 245;     // R
        buffer[i + 1] = 240; // G
        buffer[i + 2] = 230; // B (beige chiaro)
    }
    let mut framebuffer = ImageFramebuffer {
        width,
        height,
        buffer,
    };

    // Crea l'engine 3D
    let mut engine = K3dengine::new(width as u16, height as u16);
    
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

    // Raccogliamo tutti i dati prima di creare le mesh per evitare problemi di lifetime
    struct ElementData {
        id: i64,  // ID per ordinamento deterministico
        vertices: Vec<[f32; 3]>,
        lines: Vec<[usize; 2]>,
        faces: Vec<[usize; 3]>,  // Triangoli per rendering solido
        normals: Vec<[f32; 3]>,  // Normal per ogni faccia
        color: Rgb565,
        is_solid: bool,  // Se true, usa rendering solido invece di linee
        priority: u8,    // Priorità di rendering
    }
    

    let mut element_data_vec: Vec<ElementData> = Vec::new();

    // Calcola il raggio in coordinate world space per avere cerchi di dimensione pixel corretta
    let pixel_to_world = scale_factor; // Stessa scala usata per le coordinate

    // Prepara i dati per tutti gli elementi
    for elemento in elementi {
        let color = elemento.colore();
        let priority = elemento.priorita_rendering();
        let coordinate = elemento.coordinate();

        // Gestisci punti (alberi, punti interesse)
        if elemento.is_punto() {
            // Verifica se questo nodo (identificato dal suo ID) fa parte di una way
            // L'ID del MapElement per un punto corrisponde all'ID del nodo OSM originale
            if nodi_in_ways.contains(&elemento.id()) {
                // Questo punto è già parte di una linea o poligono, saltalo
                continue;
            }
            
            if let Some((lat, lon)) = coordinate.first() {
                let [x, y, _] = to_3d(*lat, *lon);
                // Raggio in pixel: varia in base al tipo
                let (radius_pixels, n_points) = match elemento.element_type {
                    ElementType::Albero => (8.0, 12),  // Alberi più grandi e visibili
                    ElementType::PuntoInteresse { .. } => (3.0, 12),  // Punti di interesse ben visibili
                    _ => (2.0, 10),  // Altri punti più piccoli
                };
                let radius = radius_pixels * pixel_to_world;

                let mut vertices = Vec::new();
                for i in 0..n_points {
                    let angle = (i as f32 / n_points as f32) * 2.0 * std::f32::consts::PI;
                    vertices.push([x + angle.cos() * radius, y + angle.sin() * radius, 0.0]);
                }

                let mut lines = Vec::new();
                for i in 0..n_points {
                    lines.push([i, (i + 1) % n_points]);
                }

                element_data_vec.push(ElementData {
                    id: elemento.id(),
                    vertices,
                    lines,
                    faces: Vec::new(),
                    normals: Vec::new(),
                    color,
                    is_solid: false,
                    priority,
                });
            }
        } else {
            // Gestisci linee e poligoni
            if coordinate.len() < 2 {
                continue;
            }

            let mut vertices = Vec::new();
            for (lat, lon) in &coordinate {
                vertices.push(to_3d(*lat, *lon));
            }

            // Per poligoni chiusi, genera triangoli per rendering solido
            let is_solid = elemento.is_chiuso() && vertices.len() >= 3;
            
            let mut lines = Vec::new();
            let mut faces = Vec::new();
            let mut normals = Vec::new();
            
            if is_solid {
                // Triangola il poligono
                faces = triangola_poligono(&vertices);
                // Per poligoni 2D (z=0), tutte le normali puntano verso l'alto (0, 0, 1)
                for _face in &faces {
                    normals.push([0.0, 0.0, 1.0]);
                }
            } else {
                // Per linee aperte, usa solo linee
                for i in 0..vertices.len() - 1 {
                    lines.push([i, i + 1]);
                }
            }

            element_data_vec.push(ElementData {
                id: elemento.id(),
                vertices,
                lines,
                faces,
                normals,
                color,
                is_solid,
                priority,
            });
        }
    }

    // Ora crea le mesh con riferimenti ai dati che vivono abbastanza a lungo
    // Creiamo le mesh e le ordiniamo per priorità e ID per garantire consistenza
    let mut meshes_with_priority: Vec<(K3dMesh, u8, i64)> = Vec::new();

    for element_data in &element_data_vec {
        if element_data.is_solid && !element_data.faces.is_empty() {
            // Rendering solido per poligoni
            let mut mesh = K3dMesh::new(Geometry {
                vertices: &element_data.vertices,
                faces: &element_data.faces,
                colors: &[],
                lines: &[],  // Non mostriamo le linee per i poligoni solidi
                normals: &element_data.normals,
            });
            mesh.set_color(element_data.color);
            mesh.set_render_mode(RenderMode::Solid);
            meshes_with_priority.push((mesh, element_data.priority, element_data.id));
        } else {
            // Rendering a linee per strade, fiumi, punti, etc.
            let mut mesh = K3dMesh::new(Geometry {
                vertices: &element_data.vertices,
                faces: &[],
                colors: &[],
                lines: &element_data.lines,
                normals: &[],
            });
            mesh.set_color(element_data.color);
            mesh.set_render_mode(RenderMode::Lines);
            meshes_with_priority.push((mesh, element_data.priority, element_data.id));
        }
    }
    
    // Ordina per priorità (più bassa prima), poi per ID per garantire consistenza
    meshes_with_priority.sort_by_key(|(_, priority, id)| (*priority, *id));
    
    // Estrai solo le mesh (ora ordinate)
    let meshes: Vec<K3dMesh> = meshes_with_priority.into_iter().map(|(mesh, _, _)| mesh).collect();

    // Renderizza tutte le mesh
    // L'API si aspetta IntoIterator<Item = &K3dMesh>, quindi passiamo &meshes
    let mut primitive_count = 0;
    engine.render(&meshes, |p| {
        primitive_count += 1;
        draw(p, &mut framebuffer);
    });
    println!("Renderizzati {} primitivi", primitive_count);

    // Converti il framebuffer in RgbImage e salva
    let img = RgbImage::from_raw(width, height, framebuffer.buffer)
        .ok_or("Failed to create image from framebuffer")?;
    
    img.save(output_path)?;
    println!("Mappa salvata in: {}", output_path);

    Ok(())
}
