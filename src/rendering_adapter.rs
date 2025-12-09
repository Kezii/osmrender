use embedded_gfx::mesh::{Geometry, K3dMesh, RenderMode};
use embedded_graphics_core::pixelcolor::Rgb565;
use crate::map_elements::{MapElement, ElementType};

/// Struttura che mantiene tutti i dati delle mesh per garantire che i riferimenti siano validi
/// Necessaria perché K3dMesh usa riferimenti ai dati
pub struct MeshData {
    /// Dati della geometria (mantenuti qui per i lifetime)
    pub vertices: Vec<[f32; 3]>,
    pub lines: Vec<[usize; 2]>,
    pub faces: Vec<[usize; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub color: Rgb565,
    pub render_mode: RenderMode,
}

/// Parametri per la conversione da coordinate geografiche a coordinate 3D
pub struct ConversionParams {
    /// Bounds geografici
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lon: f64,
    pub max_lon: f64,
    /// Dimensioni dell'immagine
    pub width: u32,
    pub height: u32,
    /// Fattore di scala per le coordinate world space
    pub scale_factor: f32,
    /// Offset Z per la priorità più bassa (elementi che stanno sotto)
    pub z_base: f32,
    /// Spaziatura tra i livelli di priorità in Z
    pub z_spacing: f32,
}

impl ConversionParams {
    /// Crea i parametri di conversione calcolando i bounds dagli elementi
    pub fn from_elements(
        elementi: &[MapElement],
        width: u32,
        height: u32,
        scale_factor: f32,
        z_base: f32,
        z_spacing: f32,
    ) -> Self {
        if elementi.is_empty() {
            return Self {
                min_lat: 0.0,
                max_lat: 0.0,
                min_lon: 0.0,
                max_lon: 0.0,
                width,
                height,
                scale_factor,
                z_base,
                z_spacing,
            };
        }

        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for elemento in elementi {
            let coordinate = elemento.coordinate();
            for (lat, lon) in coordinate {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
        }

        Self {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
            width,
            height,
            scale_factor,
            z_base,
            z_spacing,
        }
    }

    /// Converte coordinate geografiche a coordinate 3D world space
    /// priority: priorità di rendering (0 = più bassa, sotto tutto)
    fn to_3d(&self, lat: f64, lon: f64, priority: u8) -> [f32; 3] {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;
        
        let x = ((lon - self.min_lon) / (self.max_lon - self.min_lon) * self.width as f64) as f32;
        let y = ((lat - self.min_lat) / (self.max_lat - self.min_lat) * self.height as f64) as f32;
        
        let x_world = (x - center_x) * self.scale_factor;
        let y_world = (y - center_y) * self.scale_factor;
        
        // Z basato sulla priorità: priorità più alta = Z più alto (più vicino alla camera)
        let z = self.z_base + (priority as f32 * self.z_spacing);
        
        [x_world, y_world, z]
    }
}

/// Triangola un poligono usando fan triangulation
fn triangola_poligono(vertices: &[[f32; 3]]) -> Vec<[usize; 3]> {
    if vertices.len() < 3 {
        return Vec::new();
    }
    
    let mut faces = Vec::new();
    for i in 1..vertices.len() - 1 {
        faces.push([0, i, i + 1]);
    }
    faces
}

/// Container che mantiene i dati delle mesh per garantire che i riferimenti siano validi
pub struct MeshContainer {
    mesh_data: Vec<MeshData>,
}

impl MeshContainer {
    /// Crea un nuovo container con i dati delle mesh
    pub fn new(mesh_data: Vec<MeshData>) -> Self {
        Self { mesh_data }
    }

    /// Restituisce un array di mesh pronte per il rendering
    /// I riferimenti sono validi finché il container esiste
    pub fn get_meshes(&self) -> Vec<K3dMesh<'_>> {
        self.mesh_data.iter().map(|mesh_data_item| {
            let mut mesh = K3dMesh::new(Geometry {
                vertices: &mesh_data_item.vertices,
                faces: &mesh_data_item.faces,
                colors: &[],
                lines: &mesh_data_item.lines,
                normals: &mesh_data_item.normals,
            });
            mesh.set_color(mesh_data_item.color);
            // Copia manualmente RenderMode (non implementa Clone)
            match mesh_data_item.render_mode {
                RenderMode::Points => mesh.set_render_mode(RenderMode::Points),
                RenderMode::Lines => mesh.set_render_mode(RenderMode::Lines),
                RenderMode::Solid => mesh.set_render_mode(RenderMode::Solid),
                RenderMode::SolidLightDir(dir) => mesh.set_render_mode(RenderMode::SolidLightDir(dir)),
            }
            mesh
        }).collect()
    }
}

/// Converte un array di MapElement in un array ordinato di mesh pronte per il rendering
/// Usa la coordinata Z per gestire le occlusioni in base alla priorità
pub fn converti_a_mesh(
    elementi: &[MapElement],
    nodi_in_ways: &std::collections::HashSet<i64>,
    params: ConversionParams,
) -> MeshContainer {
    if elementi.is_empty() {
        return MeshContainer::new(Vec::new());
    }

    let pixel_to_world = params.scale_factor;

    // Raccogliamo tutti i dati degli elementi
    struct ElementData {
        id: i64,
        vertices: Vec<[f32; 3]>,
        lines: Vec<[usize; 2]>,
        faces: Vec<[usize; 3]>,
        normals: Vec<[f32; 3]>,
        color: Rgb565,
        is_solid: bool,
        priority: u8,
    }

    let mut element_data_vec: Vec<ElementData> = Vec::new();

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
                let [x, y, z] = params.to_3d(*lat, *lon, priority);
                let (radius_pixels, n_points) = match elemento.element_type {
                    ElementType::Albero => (8.0, 12),
                    ElementType::PuntoInteresse { .. } => (3.0, 12),
                    _ => (2.0, 10),
                };
                let radius = radius_pixels * pixel_to_world;

                let mut vertices = Vec::new();
                for i in 0..n_points {
                    let angle = (i as f32 / n_points as f32) * 2.0 * std::f32::consts::PI;
                    // Tutti i vertici del cerchio hanno lo stesso Z
                    vertices.push([x + angle.cos() * radius, y + angle.sin() * radius, z]);
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
                // Ogni vertice ha Z basato sulla priorità
                vertices.push(params.to_3d(*lat, *lon, priority));
            }

            let is_solid = elemento.is_chiuso() && vertices.len() >= 3;
            
            let mut lines = Vec::new();
            let mut faces = Vec::new();
            let mut normals = Vec::new();
            
            if is_solid {
                faces = triangola_poligono(&vertices);
                // Per poligoni 2D, tutte le normali puntano verso l'alto (0, 0, 1)
                for _face in &faces {
                    normals.push([0.0, 0.0, 1.0]);
                }
            } else {
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


    // Converti ElementData in MeshData
    let mesh_data_vec: Vec<MeshData> = element_data_vec.into_iter().map(|e| {
        let has_faces = !e.faces.is_empty();
        MeshData {
            vertices: e.vertices,
            lines: if e.is_solid { Vec::new() } else { e.lines },
            faces: e.faces,
            normals: e.normals,
            color: e.color,
            render_mode: if e.is_solid && has_faces {
                RenderMode::Solid
            } else {
                RenderMode::Lines
            },
        }
    }).collect();

    // Crea il container con i dati delle mesh
    MeshContainer::new(mesh_data_vec)
}
