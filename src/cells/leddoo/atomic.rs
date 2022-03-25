use bevy::{
    math::{ivec3, IVec3},
    tasks::{TaskPool},
};

use futures_lite::future;

use crate::{
    cell_renderer::{CellRenderer},
    rule::Rule,
    utils::{self},
};

use super::{
    CHUNK_SIZE, CHUNK_CELL_COUNT,
    index_to_chunk_index, index_to_chunk_offset,
};

use std::sync::{atomic::{AtomicU8, Ordering}, Arc, RwLock};
use std::cell::UnsafeCell;



#[derive(Default)]
struct Cell {
    value: u8,
    neighbors: UnsafeCell<AtomicU8>,
}

unsafe impl Sync for Cell {}

impl Cell {
    fn is_dead(&self) -> bool {
        self.value == 0
    }

    fn neighbors(&self) -> u8 {
        unsafe { *(*self.neighbors.get()).get_mut() }
    }

    fn neighbors_mut(&self) -> &mut u8 {
        unsafe { (*self.neighbors.get()).get_mut() }
    }

    fn neighbors_atomic(&self) -> &mut AtomicU8 {
        unsafe { &mut *self.neighbors.get() }
    }
}

type Chunk  = super::Chunk<Cell>;
type Chunks = super::Chunks<Cell>;

pub struct LeddooAtomic {
    chunks: Arc<RwLock<Chunks>>,
}

impl LeddooAtomic {
    pub fn new() -> Self {
        LeddooAtomic {
            chunks: Arc::new(RwLock::new(Chunks::new())),
        }
    }

    pub fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        let mut chunks = self.chunks.write().unwrap();
        chunks.set_bounds(new_bounds)
    }

    pub fn bounds(&self) -> i32 {
        let chunks = self.chunks.read().unwrap();
        chunks.bounds()
    }

    pub fn center(&self) -> IVec3 {
        let center = self.bounds() / 2;
        ivec3(center, center, center)
    }

    pub fn cell_count(&self) -> usize {
        let chunks = self.chunks.read().unwrap();
        let mut result = 0;
        for chunk in &chunks.chunks {
            for cell in chunk.0.iter() {
                if !cell.is_dead() {
                    result += 1;
                }
            }
        }
        result
    }


    fn update_neighbors(chunks: &Vec<Chunk>, chunk_index: usize, chunk_radius: usize,
        rule: &Rule, offset: usize, inc: bool
    ) {
        let pos = Chunks::index_to_pos_ex(chunk_index*CHUNK_CELL_COUNT + offset, chunk_radius);

        let local = Chunk::index_to_pos(offset);
        if Chunk::is_border_pos(local, 1) {
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let neighbour_pos = utils::wrap(pos + *dir, (chunk_radius*CHUNK_SIZE) as i32);

                let index  = Chunks::pos_to_index_ex(neighbour_pos, chunk_radius);
                let chunk  = index_to_chunk_index(index);
                let offset = index_to_chunk_offset(index);
                let neighbors = &*chunks[chunk].0[offset].neighbors_atomic();
                if inc {
                    neighbors.fetch_add(1, Ordering::Relaxed);
                }
                else {
                    neighbors.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
        else {
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let neighbour_pos = local + *dir;
                let offset = Chunk::pos_to_index(neighbour_pos);

                let neighbors = chunks[chunk_index].0[offset].neighbors_mut();
                if inc {
                    *neighbors += 1;
                }
                else {
                    *neighbors -= 1;
                }
            }
        }
    }

    fn update_values(chunk: &mut Chunk, rule: &Rule,
        spawns: &mut Vec<usize>, deaths: &mut Vec<usize>,
    ) {
        for (offset, cell) in chunk.0.iter_mut().enumerate() {
            if cell.is_dead() {
                if rule.birth_rule.in_range(cell.neighbors()) {
                    cell.value = rule.states;
                    spawns.push(offset);
                }
            }
            else {
                if cell.value < rule.states || !rule.survival_rule.in_range(cell.neighbors()) {
                    if cell.value == rule.states {
                        deaths.push(offset);
                    }

                    cell.value -= 1;
                }
            }
        }
    }

    pub fn update(&mut self, rule: &Rule, tasks: &TaskPool) {
        let mut chunks = self.chunks.write().unwrap();
        let chunk_radius = chunks.chunk_radius;

        let mut chunk_list = std::mem::take(&mut chunks.chunks);

        // update values.
        let mut value_tasks = vec![];
        for mut chunk in chunk_list.into_iter() {
            let rule = rule.clone(); // shrug
            let mut chunk_spawns = vec![];
            let mut chunk_deaths = vec![];

            value_tasks.push(tasks.spawn(async move {
                Self::update_values(&mut chunk, &rule,
                    &mut chunk_spawns, &mut chunk_deaths);
                (chunk, chunk_spawns, chunk_deaths)
            }));
        }

        // collect spawns & deaths.
        chunk_list = vec![];
        let mut chunk_spawns = vec![];
        let mut chunk_deaths = vec![];
        for task in value_tasks {
            let (chunk, spawns, deaths) = future::block_on(task);
            chunk_list.push(chunk);
            chunk_spawns.push(spawns);
            chunk_deaths.push(deaths);
        }

        chunks.chunks = chunk_list;
        drop(chunks);


        // update neighbors.
        let mut neighbour_tasks = vec![];
        for (chunk_index, (spawns, deaths)) in chunk_spawns.into_iter().zip(chunk_deaths).enumerate() {
            let rule = rule.clone(); // shrug

            let chunks = self.chunks.clone();

            neighbour_tasks.push(tasks.spawn(async move {
                let chunks = &chunks.read().unwrap().chunks;
                for offset in spawns.iter() {
                    Self::update_neighbors(chunks, chunk_index, chunk_radius, &rule, *offset, true);
                }

                for offset in deaths.iter() {
                    Self::update_neighbors(chunks, chunk_index, chunk_radius, &rule, *offset, false);
                }
            }));
        }

        for task in neighbour_tasks {
            future::block_on(task);
        }
    }


    // TEMP: move to sims.
    #[allow(dead_code)]
    fn validate(&self, rule: &Rule) {
        let chunks = self.chunks.read().unwrap();
        let bounds = chunks.bounds();

        for index in 0..chunks.chunk_count*CHUNK_CELL_COUNT {
            let pos = chunks.index_to_pos(index);

            let mut neighbors = 0;
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let neighbour_pos = utils::wrap(pos + *dir, bounds);

                let index  = chunks.pos_to_index(neighbour_pos);
                let chunk  = index_to_chunk_index(index);
                let offset = index_to_chunk_offset(index);
                if chunks.chunks[chunk].0[offset].value == rule.states {
                    neighbors += 1;
                }
            }

            let chunk  = index_to_chunk_index(index);
            let offset = index_to_chunk_offset(index);
            let cell   = &chunks.chunks[chunk].0[offset];
            assert_eq!(neighbors, cell.neighbors());
        }
    }

    pub fn spawn_noise(&mut self, rule: &Rule) {
        let center = self.center();
        let bounds = self.bounds();

        let mut chunks = self.chunks.write().unwrap();
        utils::make_some_noise_default(center, |pos| {
            let index  = chunks.pos_to_index(utils::wrap(pos, bounds));
            let chunk  = index_to_chunk_index(index);
            let offset = index_to_chunk_offset(index);
            let cell = &mut chunks.chunks[chunk].0[offset];
            if cell.is_dead() {
                cell.value = rule.states;
                Self::update_neighbors(
                    &chunks.chunks, chunk, chunks.chunk_radius,
                    rule, offset, true);
            }
        });
    }
}


impl crate::cells::Sim for LeddooAtomic {
    fn update(&mut self, rule: &Rule, task_pool: &TaskPool) {
        self.update(rule, task_pool);
    }

    fn render(&self, renderer: &mut CellRenderer) {
        self.chunks.read().unwrap().visit_cells(|index, cell| {
            renderer.set(index, cell.value, cell.neighbors());
        });
    }

    fn spawn_noise(&mut self, rule: &Rule) {
        self.spawn_noise(rule);
    }

    fn cell_count(&self) -> usize {
        self.cell_count()
    }

    fn bounds(&self) -> i32 {
        self.bounds()
    }

    fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        self.set_bounds(new_bounds)
    }
}

