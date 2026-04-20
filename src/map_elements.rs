use embedded_graphics_core::pixelcolor::Rgb565;

use crate::{WorldPos, chunk_manager::GeoBBox};

/// Struct che rappresenta tutti gli elementi della mappa da renderizzare,
/// indipendentemente dalla loro origine OSM (nodi, ways, poligoni)
#[derive(Clone, PartialEq, Debug, bincode::Encode, bincode::Decode)]
pub struct MapElement {
    /// ID univoco dell'elemento
    pub id: i64,
    /// Coordinate dell'elemento: (lat, lon)
    /// - Per punti: un solo elemento
    /// - Per linee: sequenza di punti
    /// - Per poligoni: sequenza di punti chiusa (anello esterno)
    pub vertices: Vec<WorldPos>,
    /// Anelli interni (buchi) per multipolygon
    /// Ogni anello interno è un vettore di coordinate (lat, lon)
    pub inner_rings: Vec<Vec<WorldPos>>,
    /// Tipo specifico dell'elemento
    pub element_type: ElementType,
}

impl MapElement {
    pub fn bbox(&self) -> GeoBBox {
        debug_assert!(
            !self.vertices.is_empty(),
            "MapElement::bbox() requires at least one vertex"
        );
        GeoBBox {
            min_lat: self
                .vertices
                .iter()
                .map(|pos| pos.lat())
                .fold(f64::INFINITY, f64::min),
            max_lat: self
                .vertices
                .iter()
                .map(|pos| pos.lat())
                .fold(f64::NEG_INFINITY, f64::max),
            min_lon: self
                .vertices
                .iter()
                .map(|pos| pos.lon())
                .fold(f64::INFINITY, f64::min),
            max_lon: self
                .vertices
                .iter()
                .map(|pos| pos.lon())
                .fold(f64::NEG_INFINITY, f64::max),
        }
    }
}
/// Enum che rappresenta il tipo specifico dell'elemento della mappa
#[derive(Clone, PartialEq, Debug, bincode::Encode, bincode::Decode)]
pub enum ElementType {
    /// Edificio (poligono chiuso)
    Edificio,
    /// Strada principale (linea aperta)
    StradaPrincipale,
    /// Strada secondaria (linea aperta)
    StradaSecondaria,
    /// Strada locale (linea aperta)
    StradaLocale,
    /// Strada pedonale (linea aperta)
    StradaPedonale,
    /// Ferrovia (linea aperta)
    Ferrovia,
    /// Fiume (linea aperta)
    Fiume,
    /// Canale (linea aperta)
    Canale,
    /// Parco (poligono chiuso)
    Parco,
    /// Acqua (poligono chiuso)
    Acqua,
    /// Foresta (poligono chiuso)
    Foresta,
    /// Boscaglia (poligono chiuso)
    Boscaglia,
    /// Area residenziale (poligono chiuso)
    Residenziale,
    /// Area commerciale (poligono chiuso)
    Commerciale,
    /// Area industriale (poligono chiuso)
    Industriale,
    /// Area agricola (poligono chiuso)
    Agricolo,
    /// Aeroporto (poligono chiuso)
    Aeroporto,
    /// Cimitero (poligono chiuso)
    Cimitero,
    /// Campo sportivo (poligono chiuso)
    CampoSportivo,
    /// Albero (punto)
    Albero,
    /// Punto di interesse (punto)
    PuntoInteresse {
        /// True se il punto ha un nome
        ha_nome: bool,
    },
    /// Altro elemento (punto o linea aperta)
    Altro {
        /// True se è un punto, false se è una linea
        is_punto: bool,
    },
    /// Bordo di un chunk (debug/overlay)
    ChunkBorder,
}

impl MapElement {
    /// Restituisce l'ID dell'elemento
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Restituisce true se l'elemento è un poligono chiuso
    pub fn is_chiuso(&self) -> bool {
        matches!(
            self.element_type,
            ElementType::Edificio
                | ElementType::Parco
                | ElementType::Acqua
                | ElementType::Foresta
                | ElementType::Boscaglia
                | ElementType::Residenziale
                | ElementType::Commerciale
                | ElementType::Industriale
                | ElementType::Agricolo
                | ElementType::Aeroporto
                | ElementType::Cimitero
                | ElementType::CampoSportivo
        )
    }

    pub fn wide_line(&self) -> Option<f32> {
        match self.element_type {
            ElementType::StradaPrincipale => Some(8.0),
            ElementType::StradaSecondaria => Some(6.0),
            ElementType::StradaLocale => Some(4.0),
            ElementType::StradaPedonale => Some(2.0),
            ElementType::Ferrovia => Some(4.0),
            _ => None,
        }
    }

    /// Restituisce true se l'elemento è una linea aperta
    pub fn is_linea_aperta(&self) -> bool {
        matches!(
            self.element_type,
            ElementType::StradaPrincipale
                | ElementType::StradaSecondaria
                | ElementType::StradaLocale
                | ElementType::StradaPedonale
                | ElementType::Ferrovia
                | ElementType::Fiume
                | ElementType::Canale
        ) || matches!(self.element_type, ElementType::Altro { is_punto } if !is_punto)
    }

    /// Restituisce true se l'elemento è un punto
    pub fn is_punto(&self) -> bool {
        matches!(
            self.element_type,
            ElementType::Albero | ElementType::PuntoInteresse { .. }
        ) || matches!(self.element_type, ElementType::Altro { is_punto } if is_punto)
    }

    /// Determina il colore per l'elemento della mappa
    pub fn colore(&self) -> Rgb565 {
        match self.element_type {
            ElementType::StradaPrincipale => Rgb565::new(200 >> 3, 80 >> 2, 40 >> 3),
            ElementType::StradaSecondaria => Rgb565::new(220 >> 3, 120 >> 2, 60 >> 3),
            ElementType::StradaLocale => Rgb565::new(180 >> 3, 180 >> 2, 180 >> 3),
            ElementType::StradaPedonale => Rgb565::new(200 >> 3, 200 >> 2, 150 >> 3),
            ElementType::Ferrovia => Rgb565::new(100 >> 3, 100 >> 2, 100 >> 3),
            ElementType::Fiume => Rgb565::new(50 >> 3, 100 >> 2, 200 >> 3),
            ElementType::Canale => Rgb565::new(80 >> 3, 150 >> 2, 220 >> 3),
            ElementType::Edificio => Rgb565::new(180 >> 3, 140 >> 2, 100 >> 3),
            ElementType::Parco => Rgb565::new(0, 31, 0),
            ElementType::Acqua => Rgb565::new(100 >> 3, 150 >> 2, 220 >> 3),
            ElementType::Foresta => Rgb565::new(0, 12, 0), // Verde più scuro per landuse:forest
            ElementType::Boscaglia => Rgb565::new(0, 25, 0),
            ElementType::Residenziale => Rgb565::new(200 >> 3, 200 >> 2, 200 >> 3),
            ElementType::Commerciale => Rgb565::new(255 >> 3, 200 >> 2, 200 >> 3),
            ElementType::Industriale => Rgb565::new(150 >> 3, 150 >> 2, 150 >> 3),
            ElementType::Agricolo => Rgb565::new(155 >> 3, 255 >> 2, 120 >> 3),
            ElementType::Aeroporto => Rgb565::new(220 >> 3, 160 >> 2, 220 >> 3),
            ElementType::Cimitero => Rgb565::new(160 >> 3, 160 >> 2, 160 >> 3),
            ElementType::CampoSportivo => Rgb565::new(0, 28, 0),
            ElementType::Albero => Rgb565::new(0, 31, 0),
            ElementType::PuntoInteresse { ha_nome } => {
                if ha_nome {
                    Rgb565::new(0, 15, 31)
                } else {
                    Rgb565::new(0, 20, 20)
                }
            }
            ElementType::Altro { is_punto } => {
                if is_punto {
                    Rgb565::new(31, 10, 0)
                } else {
                    Rgb565::new(120 >> 3, 130 >> 2, 140 >> 3)
                }
            }
            ElementType::ChunkBorder => Rgb565::new(31, 0, 0),
        }
    }

    /// Determina la priorità di rendering (più bassa = renderizzata prima, sotto)
    pub fn priorita_rendering(&self) -> u8 {
        match self.element_type {
            ElementType::Residenziale
            | ElementType::Commerciale
            | ElementType::Industriale
            | ElementType::Agricolo
            | ElementType::Aeroporto
            | ElementType::Cimitero => 0,
            ElementType::Foresta
            | ElementType::Boscaglia
            | ElementType::Parco
            | ElementType::CampoSportivo => 1,
            ElementType::Acqua => 2, // Acqua sopra foreste e parchi
            ElementType::Edificio => 3,
            ElementType::Fiume | ElementType::Canale => 4,
            ElementType::StradaPedonale => 5,
            ElementType::StradaLocale => 6,
            ElementType::StradaSecondaria => 7,
            ElementType::StradaPrincipale => 8,
            ElementType::Ferrovia => 9,
            ElementType::Albero | ElementType::PuntoInteresse { .. } => 10,
            ElementType::Altro { .. } => 1,
            ElementType::ChunkBorder => 11,
        }
    }
}
