pub mod chunk_manager;
pub mod converter;
pub mod imageframebuffer;
pub mod map_elements;
pub mod raw_osm_reader;
pub mod rendering_adapter;
pub mod renderprocess;
pub mod spatial_index;

#[derive(Clone, PartialEq, Debug, bincode::Encode, bincode::Decode, Copy)]

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

    pub fn offset_in_meters(self, other: GeoPos) -> (f64, f64) {
        let d_lat_deg = other.lat() - self.lat();
        let d_lon_deg = other.lon() - self.lon();

        let north_m = d_lat_deg * 111_000.0;
        let east_m = d_lon_deg * 111_000.0 * self.lat().to_radians().cos();

        (north_m, east_m)
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
