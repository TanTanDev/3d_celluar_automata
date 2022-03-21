mod single_threaded;
pub use single_threaded::*;

mod multi_threaded;
pub use multi_threaded::*;


#[derive(Debug)]
struct CellState {
    value: u8,
    neighbours: u8,
    dist_to_center: f32,
}

impl CellState {
    pub fn new(value: u8, neighbours: u8, dist_to_center: f32) -> Self {
        CellState {
            value,
            neighbours,
            dist_to_center,
        }
    }
}
