pub mod chunk_manager;
pub mod converter;
pub mod imageframebuffer;
pub mod map_elements;
pub mod raw_osm_reader;
pub mod rendering_adapter;
pub mod renderprocess;
pub mod spatial_index;

#[derive(Clone, PartialEq, Debug, bincode::Encode, bincode::Decode, Copy)]

pub struct WorldPos(f64, f64);

impl WorldPos {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self(lat, lon)
    }

    pub fn lat(&self) -> f64 {
        self.0
    }

    pub fn lon(&self) -> f64 {
        self.1
    }
}

impl std::ops::Add for WorldPos {
    type Output = WorldPos;

    fn add(self, other: WorldPos) -> WorldPos {
        WorldPos(self.0 + other.0, self.1 + other.1)
    }
}

impl std::ops::AddAssign for WorldPos {
    fn add_assign(&mut self, other: WorldPos) {
        self.0 += other.0;
        self.1 += other.1;
    }
}
