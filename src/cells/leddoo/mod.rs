use bevy::math::IVec3;
use crate::utils;

mod single_threaded;
pub use single_threaded::*;

mod multi_threaded;
pub use multi_threaded::*;




const CHUNK_SIZE:       usize = 32;
const CHUNK_CELL_COUNT: usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;

fn index_to_chunk_index(index: usize) -> usize {
    index / CHUNK_CELL_COUNT
}

fn index_to_chunk_offset(index: usize) -> usize {
    index % CHUNK_CELL_COUNT
}


struct Chunk<Cell> (Vec<Cell>);

impl<Cell: Default> Default for Chunk<Cell> {
    fn default() -> Chunk<Cell> {
        let cells =
            (0..CHUNK_CELL_COUNT)
            .map(|_| Cell::default())
            .collect::<Vec<_>>();
        Chunk(cells)
    }
}

impl<Cell> Chunk<Cell> {
    fn index_to_pos(index: usize) -> IVec3 {
        utils::index_to_pos(index, CHUNK_SIZE as i32)
    }

    fn pos_to_index(pos: IVec3) -> usize {
        utils::pos_to_index(pos, CHUNK_SIZE as i32)
    }

    fn is_border_pos(pos: IVec3, offset: i32) -> bool {
        pos.x - offset <= 0 || pos.x + offset >= CHUNK_SIZE as i32 - 1 ||
        pos.y - offset <= 0 || pos.y + offset >= CHUNK_SIZE as i32 - 1 ||
        pos.z - offset <= 0 || pos.z + offset >= CHUNK_SIZE as i32 - 1
    }
}
