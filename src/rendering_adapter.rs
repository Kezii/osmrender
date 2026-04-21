use crate::{
    GeoBBox, GeoPos,
    map_elements::{ElementType, MapElement},
};
use earcut::Earcut;
use embedded_gfx::mesh::{Geometry, K3dMesh, RenderMode};
use embedded_graphics_core::pixelcolor::Rgb565;
use log::debug;

/// Struttura che mantiene tutti i dati delle mesh per garantire che i riferimenti siano validi
/// Necessaria perché K3dMesh usa riferimenti ai dati
pub struct OwnedMeshData {
    /// Dati della geometria (mantenuti qui per i lifetime)
    pub vertices: Vec<[f32; 3]>,
    pub lines: Vec<[usize; 2]>,
    pub faces: Vec<[usize; 3]>,
    pub color: Rgb565,
    pub render_mode: RenderMode,
    pub priority: u8,
    pub bbox: GeoBBox,
}

impl OwnedMeshData {
    pub fn to_kmesh(&'_ self) -> K3dMesh<'_> {
        let mut mesh = K3dMesh::new(Geometry {
            vertices: &self.vertices,
            faces: &self.faces,
            colors: &[],
            lines: &self.lines,
            normals: &[],
        });
        mesh.set_color(self.color);
        mesh.set_render_mode(self.render_mode.clone());
        mesh
    }
}

/// Parametri per la conversione da coordinate geografiche a coordinate 3D
pub struct MapToMeshConversionParams {
    /// this becomes the 0,0 point of the world
    pub center_offset: GeoPos,
    /// Fattore di scala per le coordinate world space
    pub scale_factor: f64,
    /// Offset Z per la priorità più bassa (elementi che stanno sotto)
    pub z_base: f32,
    /// Spaziatura tra i livelli di priorità in Z
    pub z_spacing: f32,

    pub force_wireframe: bool,
}

impl MapToMeshConversionParams {
    /// Converte coordinate geografiche a coordinate 3D world space
    /// priority: priorità di rendering (0 = più bassa, sotto tutto)
    fn to_3d(&self, pos: &GeoPos, priority: u8) -> [f32; 3] {
        let (north_m, east_m) = self.center_offset.offset_in_meters(*pos);

        let x_world = east_m * self.scale_factor;
        let y_world = north_m * self.scale_factor;

        // Z basato sulla priorità: priorità più alta = Z più alto (più vicino alla camera)
        let z = self.z_base + (priority as f32 * self.z_spacing);

        [x_world as f32, y_world as f32, z]
    }
}

/// Triangola un poligono con buchi usando earcut (algoritmo Ear Clipping)
/// Gestisce correttamente poligoni convessi e concavi con buchi
/// outer_ring: anello esterno del poligono
/// inner_rings: anelli interni (buchi)
/// Restituisce (faces, all_vertices) dove all_vertices contiene tutti i vertici nell'ordine corretto
fn triangola_poligono_con_buchi(
    outer_ring: &[[f32; 3]],
    inner_rings: &[Vec<[f32; 3]>],
) -> (Vec<[usize; 3]>, Vec<[f32; 3]>) {
    if outer_ring.len() < 3 {
        eprintln!(
            "⚠️  triangola_poligono_con_buchi: troppo pochi vertici nell'anello esterno ({})",
            outer_ring.len()
        );
        return (Vec::new(), Vec::new());
    }

    // Rimuovi vertici duplicati (primo = ultimo) se presenti
    let mut outer_clean = outer_ring.to_vec();
    const EPSILON: f32 = 1e-6;
    if outer_clean.len() > 3 {
        let first = &outer_clean[0];
        let last = &outer_clean[outer_clean.len() - 1];
        if (first[0] - last[0]).abs() < EPSILON && (first[1] - last[1]).abs() < EPSILON {
            outer_clean.pop();
        }
    }

    if outer_clean.len() < 3 {
        eprintln!(
            "⚠️  triangola_poligono_con_buchi: dopo pulizia, troppo pochi vertici ({})",
            outer_clean.len()
        );
        return (Vec::new(), Vec::new());
    }

    // Estrai solo le coordinate X e Y (ignorando Z che è costante per ogni poligono)
    // earcut richiede un iteratore di [f64; 2]
    let mut all_vertices_2d: Vec<[f64; 2]> = outer_clean
        .iter()
        .map(|v| [v[0] as f64, v[1] as f64])
        .collect();

    // Pulisci e aggiungi gli anelli interni
    // Gli anelli interni devono essere in senso opposto rispetto all'anello esterno
    let mut inner_rings_clean_2d: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut inner_rings_clean_3d: Vec<Vec<[f32; 3]>> = Vec::new();

    // Calcola l'orientamento dell'anello esterno
    let mut outer_signed_area = 0.0;
    for i in 0..all_vertices_2d.len() {
        let j = (i + 1) % all_vertices_2d.len();
        outer_signed_area += all_vertices_2d[i][0] * all_vertices_2d[j][1];
        outer_signed_area -= all_vertices_2d[j][0] * all_vertices_2d[i][1];
    }
    let outer_is_ccw = outer_signed_area > 0.0;

    for inner_ring in inner_rings {
        let mut inner_clean_2d = inner_ring
            .iter()
            .map(|v| [v[0] as f64, v[1] as f64])
            .collect::<Vec<_>>();
        let mut inner_clean_3d = inner_ring.to_vec();

        // Rimuovi vertici duplicati
        if inner_clean_2d.len() > 3 {
            let first = &inner_clean_2d[0];
            let last = &inner_clean_2d[inner_clean_2d.len() - 1];
            if (first[0] - last[0]).abs() < EPSILON as f64
                && (first[1] - last[1]).abs() < EPSILON as f64
            {
                inner_clean_2d.pop();
                inner_clean_3d.pop();
            }
        }

        if inner_clean_2d.len() >= 3 {
            // Verifica l'orientamento dell'anello interno
            let mut inner_signed_area = 0.0;
            for i in 0..inner_clean_2d.len() {
                let j = (i + 1) % inner_clean_2d.len();
                inner_signed_area += inner_clean_2d[i][0] * inner_clean_2d[j][1];
                inner_signed_area -= inner_clean_2d[j][0] * inner_clean_2d[i][1];
            }
            let inner_is_ccw = inner_signed_area > 0.0;

            // Gli anelli interni devono avere orientamento opposto all'anello esterno
            // Se l'anello esterno è CCW, gli interni devono essere CW (e viceversa)
            if inner_is_ccw == outer_is_ccw {
                inner_clean_2d.reverse();
                inner_clean_3d.reverse();
            }

            inner_rings_clean_2d.push(inner_clean_2d);
            inner_rings_clean_3d.push(inner_clean_3d);
        }
    }

    // Calcola gli indici dei buchi (dove iniziano gli anelli interni)
    let mut hole_indices: Vec<u32> = Vec::new();
    let mut current_index = all_vertices_2d.len() as u32;

    for inner_ring in &inner_rings_clean_2d {
        hole_indices.push(current_index);
        all_vertices_2d.extend(inner_ring.iter().copied());
        current_index += inner_ring.len() as u32;
    }

    // Triangola usando earcut con buchi
    let mut triangles = Vec::new();
    let mut earcut = Earcut::new();

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        earcut.earcut(
            all_vertices_2d.iter().copied(),
            &hole_indices,
            &mut triangles,
        );
    }));
    if res.is_err() {
        debug!(
            "⚠️  triangola_poligono_con_buchi: earcut ha panicato (outer_len={}, holes={}, total_vertices={}), fallback a Lines",
            outer_clean.len(),
            inner_rings_clean_2d.len(),
            all_vertices_2d.len()
        );
        return (Vec::new(), Vec::new());
    }

    if triangles.is_empty() {
        debug!(
            "⚠️  triangola_poligono_con_buchi: earcut ha restituito 0 triangoli per {} vertici esterni e {} buchi",
            outer_clean.len(),
            inner_rings_clean_2d.len()
        );
        return (Vec::new(), Vec::new());
    }

    if triangles.len() % 3 != 0 {
        debug!(
            "⚠️  triangola_poligono_con_buchi: numero di indici non multiplo di 3: {}",
            triangles.len()
        );
    }

    // Converti gli indici u32 in [usize; 3]
    let mut faces = Vec::with_capacity(triangles.len() / 3);
    for i in (0..triangles.len()).step_by(3) {
        if i + 2 < triangles.len() {
            faces.push([
                triangles[i] as usize,
                triangles[i + 1] as usize,
                triangles[i + 2] as usize,
            ]);
        }
    }

    if faces.is_empty() {
        debug!(
            "⚠️  triangola_poligono_con_buchi: nessuna faccia generata da {} triangoli",
            triangles.len()
        );
    }

    // Costruisci il vettore completo di vertici 3D nello stesso ordine usato per la triangolazione
    // (esterno + buchi, con orientamento corretto)
    let mut all_vertices_3d = outer_clean.clone();
    for inner_ring in &inner_rings_clean_3d {
        all_vertices_3d.extend(inner_ring.iter().copied());
    }

    (faces, all_vertices_3d)
}

/// Triangola un poligono semplice (senza buchi) usando earcut (algoritmo Ear Clipping)
/// Gestisce correttamente poligoni convessi e concavi
fn triangola_poligono(vertices: &[[f32; 3]]) -> Vec<[usize; 3]> {
    if vertices.len() < 3 {
        debug!(
            "⚠️  triangola_poligono: troppo pochi vertici ({})",
            vertices.len()
        );
        return Vec::new();
    }

    // Rimuovi vertici duplicati (primo = ultimo) se presenti
    let mut vertices_clean = vertices.to_vec();
    const EPSILON: f32 = 1e-6;
    if vertices_clean.len() > 3 {
        let first = &vertices_clean[0];
        let last = &vertices_clean[vertices_clean.len() - 1];
        if (first[0] - last[0]).abs() < EPSILON && (first[1] - last[1]).abs() < EPSILON {
            vertices_clean.pop();
        }
    }

    if vertices_clean.len() < 3 {
        debug!(
            "⚠️  triangola_poligono: dopo pulizia, troppo pochi vertici ({})",
            vertices_clean.len()
        );
        return Vec::new();
    }

    // Estrai solo le coordinate X e Y (ignorando Z che è costante per ogni poligono)
    // earcut richiede un iteratore di [f64; 2]
    let vertices_2d: Vec<[f64; 2]> = vertices_clean
        .iter()
        .map(|v| [v[0] as f64, v[1] as f64])
        .collect();

    // Nessun buco nel poligono (hole_indices vuoto)
    let hole_indices: &[u32] = &[];

    // Triangola usando earcut
    let mut triangles = Vec::new();
    let mut earcut = Earcut::new();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        earcut.earcut(vertices_2d.iter().copied(), hole_indices, &mut triangles);
    }));
    if res.is_err() {
        debug!(
            "⚠️  triangola_poligono: earcut ha panicato (len={}), fallback a Lines",
            vertices_2d.len()
        );
        return Vec::new();
    }

    if triangles.is_empty() {
        debug!(
            "⚠️  triangola_poligono: earcut ha restituito 0 triangoli per {} vertici",
            vertices_2d.len()
        );
        debug!(
            "   Primi 3 vertici 2D: {:?}",
            vertices_2d.iter().take(3).collect::<Vec<_>>()
        );
        return Vec::new();
    }

    if triangles.len() % 3 != 0 {
        debug!(
            "⚠️  triangola_poligono: numero di indici non multiplo di 3: {}",
            triangles.len()
        );
    }

    // Converti gli indici u32 in [usize; 3]
    let mut faces = Vec::with_capacity(triangles.len() / 3);
    for i in (0..triangles.len()).step_by(3) {
        if i + 2 < triangles.len() {
            faces.push([
                triangles[i] as usize,
                triangles[i + 1] as usize,
                triangles[i + 2] as usize,
            ]);
        }
    }

    if faces.is_empty() {
        debug!(
            "⚠️  triangola_poligono: nessuna faccia generata da {} triangoli",
            triangles.len()
        );
    }

    faces
}

fn sottrai_2d(a: [f32; 3], b: [f32; 3]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn lunghezza_2d(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn normalizza_2d(v: [f32; 2]) -> Option<[f32; 2]> {
    let len = lunghezza_2d(v);
    if len <= 1e-6 {
        None
    } else {
        Some([v[0] / len, v[1] / len])
    }
}

fn normale_sinistra(direction: [f32; 2]) -> [f32; 2] {
    [-direction[1], direction[0]]
}

/// Triangola una polilinea aperta in una mesh solida con spessore costante.
/// Ogni nodo genera una coppia di vertici left/right e le giunzioni usano una
/// miter join limitata per evitare picchi eccessivi sugli angoli stretti.
fn triangola_linea(centerline: &[[f32; 3]], width: f32) -> (Vec<[f32; 3]>, Vec<[usize; 3]>) {
    if centerline.len() < 2 || width <= 0.0 {
        return (Vec::new(), Vec::new());
    }

    let half_width = width * 0.5;
    let mut segment_dirs = Vec::with_capacity(centerline.len().saturating_sub(1));
    let mut segment_normals = Vec::with_capacity(centerline.len().saturating_sub(1));

    for segment in centerline.windows(2) {
        let delta = sottrai_2d(segment[1], segment[0]);
        let Some(dir) = normalizza_2d(delta) else {
            continue;
        };
        segment_dirs.push(dir);
        segment_normals.push(normale_sinistra(dir));
    }

    if segment_dirs.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut expanded_vertices = Vec::with_capacity(centerline.len() * 2);
    const MITER_LIMIT: f32 = 4.0;

    for i in 0..centerline.len() {
        let point = centerline[i];
        let z = point[2];

        let (dir, normal, miter_scale) = if i == 0 {
            (segment_dirs[0], segment_normals[0], 1.0)
        } else if i == centerline.len() - 1 {
            let last = segment_dirs.len() - 1;
            (segment_dirs[last], segment_normals[last], 1.0)
        } else {
            let prev_dir = segment_dirs[i - 1];
            let next_dir = segment_dirs[i];
            let prev_normal = segment_normals[i - 1];
            //let next_normal = segment_normals[i];

            let tangent =
                match normalizza_2d([prev_dir[0] + next_dir[0], prev_dir[1] + next_dir[1]]) {
                    Some(tangent) => tangent,
                    None => prev_dir,
                };
            let miter = normale_sinistra(tangent);
            let denominator = miter[0] * prev_normal[0] + miter[1] * prev_normal[1];
            let scale = if denominator.abs() <= 1e-3 {
                1.0
            } else {
                (1.0 / denominator.abs()).min(MITER_LIMIT)
            };

            (tangent, miter, scale)
        };

        let mut center = [point[0], point[1]];
        if i == 0 {
            center[0] -= dir[0] * half_width;
            center[1] -= dir[1] * half_width;
        } else if i == centerline.len() - 1 {
            center[0] += dir[0] * half_width;
            center[1] += dir[1] * half_width;
        }

        let offset = [
            normal[0] * half_width * miter_scale,
            normal[1] * half_width * miter_scale,
        ];

        expanded_vertices.push([center[0] + offset[0], center[1] + offset[1], z]);
        expanded_vertices.push([center[0] - offset[0], center[1] - offset[1], z]);
    }

    let mut faces = Vec::with_capacity((centerline.len() - 1) * 2);
    for i in 0..centerline.len() - 1 {
        let base = i * 2;
        faces.push([base, base + 1, base + 2]);
        faces.push([base + 1, base + 3, base + 2]);
    }

    (expanded_vertices, faces)
}

impl MapElement {
    /// Converte un array di MapElement in un array ordinato di mesh pronte per il rendering
    /// Usa la coordinata Z per gestire le occlusioni in base alla priorità
    pub fn converti_a_mesh(&self, params: &MapToMeshConversionParams) -> Option<OwnedMeshData> {
        // Raccogliamo tutti i dati degli elementi
        struct ElementData {
            vertices: Vec<[f32; 3]>,
            lines: Vec<[usize; 2]>,
            faces: Vec<[usize; 3]>,
            color: Rgb565,
            is_solid: bool,
            priority: u8,
            bbox: GeoBBox,
        }

        impl ElementData {
            pub fn to_owned_mesh_data(&self, params: &MapToMeshConversionParams) -> OwnedMeshData {
                {
                    let has_faces = !self.faces.is_empty();
                    OwnedMeshData {
                        vertices: self.vertices.clone(),
                        // Per poligoni solidi con triangoli, non mostrare le linee (usa solo il riempimento)
                        // Per altri, mostra le linee normali
                        lines: if self.is_solid && has_faces {
                            Vec::new() // Poligoni solidi usano il riempimento, non le linee
                        } else {
                            self.lines.clone()
                        },
                        faces: self.faces.clone(),
                        color: self.color,
                        // Usa Solid per poligoni solidi con triangoli, Lines per il resto
                        render_mode: if self.is_solid && has_faces && !params.force_wireframe {
                            RenderMode::Solid
                        } else {
                            RenderMode::Lines
                        },
                        priority: self.priority,
                        bbox: self.bbox.clone(),
                    }
                }
            }
        }

        let mut element_data = None;

        let color = self.color_theme_gta()?;
        let priority = self.priorita_rendering();

        // Gestisci punti (alberi, punti interesse)
        if self.is_punto() {
            if let Some(pos) = self.vertices.first() {
                let [x, y, z] = params.to_3d(pos, priority);

                let radius_meters = match self.element_type {
                    ElementType::Albero => 5.0,
                    ElementType::PuntoInteresse { .. } => 3.0,
                    _ => 2.0,
                };
                let radius = radius_meters * params.scale_factor as f32;

                let n_points = 12;

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

                element_data = Some(ElementData {
                    vertices,
                    lines,
                    faces: Vec::new(),
                    color,
                    is_solid: false,
                    priority,
                    bbox: self.bbox().clone(),
                });
            }
        } else {
            // Gestisci linee e poligoni
            if self.vertices.len() < 2 {
                return None;
            }

            let mut vertices = Vec::new();
            for pos in &self.vertices {
                // Ogni vertice ha Z basato sulla priorità
                let vertex = params.to_3d(pos, priority);
                vertices.push(vertex);
            }

            let mut is_solid = self.is_chiuso() && vertices.len() >= 3;

            let mut lines = Vec::new();
            let mut faces = Vec::new();

            if is_solid {
                // Converti gli inner_rings (buchi) da coordinate geografiche a 3D
                let inner_rings_3d: Vec<Vec<[f32; 3]>> = self
                    .inner_rings
                    .iter()
                    .map(|inner_ring| {
                        inner_ring
                            .iter()
                            .map(|pos| params.to_3d(pos, priority))
                            .collect()
                    })
                    .collect();

                // Usa triangola_poligono_con_buchi se ci sono buchi, altrimenti triangola_poligono
                if !inner_rings_3d.is_empty() {
                    // Triangola usando l'anello esterno e i buchi
                    // La funzione restituisce (faces, all_vertices) con i vertici nell'ordine corretto
                    let (faces_result, all_vertices) =
                        triangola_poligono_con_buchi(&vertices, &inner_rings_3d);
                    faces = faces_result;

                    // Se la triangolazione ha successo, usa i vertici combinati
                    // perché gli indici delle faces si riferiscono a questo ordine
                    if !faces.is_empty() {
                        vertices = all_vertices;
                    }
                } else {
                    faces = triangola_poligono(&vertices);
                }

                // Se la triangolazione fallisce, usa le linee del perimetro come fallback
                if faces.is_empty() {
                    debug!(
                        "⚠️  Triangolazione fallita per elemento ID {} (tipo: {:?}), {} vertici, {} buchi",
                        self.id(),
                        self.element_type,
                        vertices.len(),
                        inner_rings_3d.len()
                    );
                    debug!(
                        "   Vertici: {:?}",
                        vertices.iter().take(5).collect::<Vec<_>>()
                    );
                    is_solid = false;
                    for i in 0..vertices.len() - 1 {
                        lines.push([i, i + 1]);
                    }
                    // Chiudi il poligono
                    if vertices.len() > 2 {
                        lines.push([vertices.len() - 1, 0]);
                    }
                }
            } else {
                if let Some(line_width) = self.wide_line() {
                    let mesh_width = line_width * params.scale_factor as f32;
                    let (triangulated_vertices, triangulated_faces) =
                        triangola_linea(&vertices, mesh_width);

                    if !triangulated_faces.is_empty() {
                        vertices = triangulated_vertices;
                        faces = triangulated_faces;
                        is_solid = true;
                    } else {
                        for i in 0..vertices.len() - 1 {
                            lines.push([i, i + 1]);
                        }
                    }
                } else {
                    for i in 0..vertices.len() - 1 {
                        lines.push([i, i + 1]);
                    }
                }
            }

            element_data = Some(ElementData {
                vertices,
                lines,
                faces,
                color,
                is_solid, // Potrebbe essere stato cambiato a false se la triangolazione fallisce
                priority,
                bbox: self.bbox().clone(),
            });
        }

        element_data.map(|e| e.to_owned_mesh_data(params))
    }
}
