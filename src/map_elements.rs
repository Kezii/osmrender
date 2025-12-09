use embedded_graphics_core::pixelcolor::Rgb565;

/// Struct che rappresenta tutti gli elementi della mappa da renderizzare,
/// indipendentemente dalla loro origine OSM (nodi, ways, poligoni)
#[derive(Clone, PartialEq, Debug)]
pub struct MapElement {
    /// ID univoco dell'elemento
    pub id: i64,
    /// Coordinate dell'elemento: (lat, lon)
    /// - Per punti: un solo elemento
    /// - Per linee: sequenza di punti
    /// - Per poligoni: sequenza di punti chiusa
    pub vertices: Vec<(f64, f64)>,
    /// Tipo specifico dell'elemento
    pub element_type: ElementType,
}

/// Enum che rappresenta il tipo specifico dell'elemento della mappa
#[derive(Clone, PartialEq, Debug)]
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

    /// Restituisce tutte le coordinate dell'elemento come (lat, lon)
    pub fn coordinate(&self) -> Vec<(f64, f64)> {
        self.vertices.clone()
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
            ElementType::Foresta => Rgb565::new(0, 20, 0),
            ElementType::Boscaglia => Rgb565::new(0, 25, 0),
            ElementType::Residenziale => Rgb565::new(200 >> 3, 200 >> 2, 200 >> 3),
            ElementType::Commerciale => Rgb565::new(255 >> 3, 200 >> 2, 200 >> 3),
            ElementType::Industriale => Rgb565::new(150 >> 3, 150 >> 2, 150 >> 3),
            ElementType::Agricolo => Rgb565::new(255 >> 3, 255 >> 2, 220 >> 3),
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
        }
    }

    /// Determina la priorità di rendering (più bassa = renderizzata prima, sotto)
    pub fn priorita_rendering(&self) -> u8 {
        match self.element_type {
            ElementType::Residenziale | ElementType::Commerciale | 
            ElementType::Industriale | ElementType::Agricolo | 
            ElementType::Aeroporto | ElementType::Cimitero => 0,
            ElementType::Acqua | ElementType::Foresta | ElementType::Boscaglia | ElementType::Parco | ElementType::CampoSportivo => 1,
            ElementType::Edificio => 2,
            ElementType::StradaPrincipale | ElementType::StradaSecondaria | 
            ElementType::StradaLocale | ElementType::StradaPedonale | 
            ElementType::Ferrovia | ElementType::Fiume | ElementType::Canale => 3,
            ElementType::Albero | ElementType::PuntoInteresse { .. } => 4,
            ElementType::Altro { .. } => 1,
        }
    }
}

