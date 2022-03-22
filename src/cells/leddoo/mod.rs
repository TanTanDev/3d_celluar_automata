use bevy::math::IVec3;
use crate::utils;

mod single_threaded;
pub use single_threaded::*;

mod multi_threaded;
pub use multi_threaded::*;

mod atomic;
pub use atomic::*;




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


struct Chunks<Cell> {
    chunks: Vec<Chunk<Cell>>,
    chunk_radius: usize,
    chunk_count:  usize,
}

impl<Cell> Chunks<Cell> {
    pub fn new() -> Chunks<Cell> {
        Chunks {
            chunks: vec![],
            chunk_radius: 0,
            chunk_count:  0,
        }
    }

    pub fn bounds(&self) -> i32 {
        (self.chunk_radius * CHUNK_SIZE) as i32
    }


    fn index_to_pos_ex(index: usize, chunk_radius: usize) -> IVec3 {
        let chunk      = index_to_chunk_index(index);
        let offset     = index_to_chunk_offset(index);
        let chunk_vec  = utils::index_to_pos(chunk, chunk_radius as i32);
        let offset_vec = Chunk::<Cell>::index_to_pos(offset);

        (CHUNK_SIZE as i32 * chunk_vec) + offset_vec
    }

    fn pos_to_index_ex(vec: IVec3, chunk_radius: usize) -> usize {
        let chunk_vec  = vec / CHUNK_SIZE as i32;
        let offset_vec = vec % CHUNK_SIZE as i32;

        let chunk  = utils::pos_to_index(chunk_vec, chunk_radius as i32);
        let offset = Chunk::<Cell>::pos_to_index(offset_vec);
        chunk*CHUNK_CELL_COUNT + offset
    }

    fn index_to_pos(&self, index: usize) -> IVec3 {
        Chunks::<Cell>::index_to_pos_ex(index, self.chunk_radius)
    }

    fn pos_to_index(&self, pos: IVec3) -> usize {
        Chunks::<Cell>::pos_to_index_ex(pos, self.chunk_radius)
    }

    fn visit_cells<F: FnMut(usize, &Cell)>(&self, mut f: F) {
        for (chunk_index, chunk) in self.chunks.iter().enumerate() {
            let chunk_pos = utils::index_to_pos(chunk_index, self.chunk_radius as i32);
            let chunk_pos = CHUNK_SIZE as i32 * chunk_pos;

            for (cell_index, cell) in chunk.0.iter().enumerate() {
                let pos = chunk_pos + Chunk::<Cell>::index_to_pos(cell_index);
                let index = utils::pos_to_index(pos, self.bounds());
                f(index, cell);
            }
        }
    }
}

impl<Cell: Default> Chunks<Cell> {
    pub fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        let radius = (new_bounds as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;

        if radius != self.chunk_radius {
            let count = radius*radius*radius;
            self.chunks.resize_with(count, || Chunk::default());
            self.chunk_radius = radius;
            self.chunk_count  = count;
        }

        self.bounds()
    }
}
