mod single_threaded;
pub use single_threaded::*;

mod multi_threaded;
pub use multi_threaded::*;


#[derive(Debug)]
struct CellState {
    value: u8,
    neighbours: u8,
}

impl CellState {
    pub fn new(value: u8, neighbours: u8) -> Self {
        CellState {
            value,
            neighbours,
        }
    }
}
