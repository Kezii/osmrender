use geo::{Point, Rect};

pub mod chunk_manager;
pub mod converter;
pub mod imageframebuffer;
pub mod map_elements;
pub mod raw_osm_reader;
pub mod rendering_adapter;
pub mod renderprocess;
pub mod spatial_index;

// ---------------------------
// WebMercator helpers
// ---------------------------

const EARTH_RADIUS_M: f64 = 6_378_137.0;
const MAX_MERCATOR_LAT: f64 = 85.051_128_78;

#[derive(Clone, PartialEq, Debug, bincode::Encode, bincode::Decode, Copy, Default)]

pub struct GeoPos(f64, f64);

impl GeoPos {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self(lat, lon)
    }

    pub fn lat(&self) -> f64 {
        self.0
    }

    pub fn lon(&self) -> f64 {
        self.1
    }

    pub fn to_geo(&self) -> Point<f64> {
        Point::new(self.1, self.0)
    }

    pub fn offset_in_meters(self, other: GeoPos) -> (f64, f64) {
        let d_lat_deg = other.lat() - self.lat();
        let d_lon_deg = other.lon() - self.lon();

        let north_m = d_lat_deg * 111_000.0;
        let east_m = d_lon_deg * 111_000.0 * self.lat().to_radians().cos();

        (north_m, east_m)
    }

    pub fn to_webmercator(self) -> (f64, f64) {
        let x = EARTH_RADIUS_M * self.lon().to_radians();
        let lat = self
            .lat()
            .clamp(-MAX_MERCATOR_LAT, MAX_MERCATOR_LAT)
            .to_radians();
        let y = EARTH_RADIUS_M * (std::f64::consts::FRAC_PI_4 + lat * 0.5).tan().ln();

        (x, y)
    }

    pub fn from_webmercator(x: f64, y: f64) -> Self {
        let lon = (x / EARTH_RADIUS_M).to_degrees();
        let lat = 2.0 * (y / EARTH_RADIUS_M).exp().atan() - std::f64::consts::FRAC_PI_2;
        Self::new(lat.to_degrees(), lon)
    }
}

impl std::ops::Add for GeoPos {
    type Output = GeoPos;

    fn add(self, other: GeoPos) -> GeoPos {
        GeoPos(self.0 + other.0, self.1 + other.1)
    }
}

impl std::ops::AddAssign for GeoPos {
    fn add_assign(&mut self, other: GeoPos) {
        self.0 += other.0;
        self.1 += other.1;
    }
}

// ---------------------------

/// Bounding box geografica (lat/lon).
#[derive(Default, Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct GeoBBox {
    pub min: GeoPos,
    pub max: GeoPos,
}

impl GeoBBox {
    pub fn normalized(self) -> Self {
        let min_lat = self.min.lat().min(self.max.lat());
        let max_lat = self.min.lat().max(self.max.lat());
        let min_lon = self.min.lon().min(self.max.lon());
        let max_lon = self.min.lon().max(self.max.lon());

        Self {
            min: GeoPos::new(min_lat, min_lon),
            max: GeoPos::new(max_lat, max_lon),
        }
    }

    #[inline]
    pub fn contains(&self, pos: GeoPos) -> bool {
        pos.lat() >= self.min.lat()
            && pos.lat() <= self.max.lat()
            && pos.lon() >= self.min.lon()
            && pos.lon() <= self.max.lon()
    }

    #[inline]
    pub fn intersects(&self, other: &GeoBBox) -> bool {
        // Assumiamo bbox normalizzate; per sicurezza normalizziamo localmente.
        let a = (*self).clone().normalized();
        let b = other.clone().normalized();
        !(a.max.lat() < b.min.lat()
            || a.min.lat() > b.max.lat()
            || a.max.lon() < b.min.lon()
            || a.min.lon() > b.max.lon())
    }

    pub fn to_geo_rect(&self) -> Rect<f64> {
        Rect::new(self.min.to_geo(), self.max.to_geo())
    }
}
