use bevy::{
    input::Input,
    math::{ivec3, IVec3},
    prelude::{KeyCode},
    tasks::{TaskPool},
};

use futures_lite::future;

use crate::{
    cell_renderer::{InstanceData},
    rule::Rule,
    utils::{self},
};

use super::{
    CHUNK_CELL_COUNT,
    index_to_chunk_index, index_to_chunk_offset,
};



#[derive(Clone, Copy, Default)]
struct Cell {
    value: u8,
    neighbours: u8,
}

impl Cell {
    fn is_dead(self) -> bool {
        self.value == 0
    }
}


type Chunk  = super::Chunk<Cell>;
type Chunks = super::Chunks<Cell>;

pub struct LeddooMultiThreaded {
    chunks: Chunks,
}

impl LeddooMultiThreaded {
    pub fn new() -> Self {
        LeddooMultiThreaded { chunks: Chunks::new() }
    }

    pub fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        self.chunks.set_bounds(new_bounds)
    }

    pub fn bounds(&self) -> i32 {
        self.chunks.bounds()
    }

    pub fn center(&self) -> IVec3 {
        let center = self.bounds() / 2;
        ivec3(center, center, center)
    }

    pub fn cell_count(&self) -> usize {
        let mut result = 0;
        for chunk in &self.chunks.chunks {
            for cell in chunk.0.iter() {
                if !cell.is_dead() {
                    result += 1;
                }
            }
        }
        result
    }


    fn wrap(&self, pos: IVec3) -> IVec3 {
        utils::wrap(pos, self.bounds())
    }


    fn update_neighbors_chunk(chunk: &mut Chunk, rule: &Rule, offset: usize, inc: bool) {
        let pos = Chunk::index_to_pos(offset);
        for dir in rule.neighbour_method.get_neighbour_iter() {
            let neighbour_pos = pos + *dir;

            let index = Chunk::pos_to_index(neighbour_pos);
            if inc {
                chunk.0[index].neighbours += 1;
            }
            else {
                chunk.0[index].neighbours -= 1;
            }
        }
    }

    fn update_neighbors(&self, chunks: &mut Vec<Chunk>, rule: &Rule, index: usize, inc: bool) {
        let pos = self.chunks.index_to_pos(index);
        for dir in rule.neighbour_method.get_neighbour_iter() {
            let neighbor_pos = self.wrap(pos + *dir);

            let index  = self.chunks.pos_to_index(neighbor_pos);
            let chunk  = index_to_chunk_index(index);
            let offset = index_to_chunk_offset(index);
            if inc {
                chunks[chunk].0[offset].neighbours += 1;
            }
            else {
                chunks[chunk].0[offset].neighbours -= 1;
            }
        }
    }

    fn update_values_chunk(chunk: &mut Chunk, chunk_index: usize, rule: &Rule,
        chunk_spawns: &mut Vec<usize>, spawns: &mut Vec<usize>,
        chunk_deaths: &mut Vec<usize>, deaths: &mut Vec<usize>,
    ) {
        for (offset, cell) in chunk.0.iter_mut().enumerate() {
            if cell.is_dead() {
                if rule.birth_rule.in_range(cell.neighbours) {
                    cell.value = rule.states;

                    if Chunk::is_border_pos(Chunk::index_to_pos(offset), 0) {
                        spawns.push(chunk_index*CHUNK_CELL_COUNT + offset);
                    }
                    else {
                        chunk_spawns.push(offset);
                    }
                }
            }
            else {
                if cell.value < rule.states || !rule.survival_rule.in_range(cell.neighbours) {
                    if cell.value == rule.states {
                        if Chunk::is_border_pos(Chunk::index_to_pos(offset), 0) {
                            deaths.push(chunk_index*CHUNK_CELL_COUNT + offset);
                        }
                        else {
                            chunk_deaths.push(offset);
                        }
                    }

                    cell.value -= 1;
                }
            }
        }
    }


    pub fn update(&mut self, rule: &Rule, tasks: &TaskPool) {
        let mut chunks = std::mem::take(&mut self.chunks.chunks);

        // update values.
        let mut value_tasks = vec![];
        for (chunk_index, mut chunk) in chunks.into_iter().enumerate()
        {
            let rule = rule.clone(); // shrug
            let mut chunk_spawns = vec![];
            let mut chunk_deaths = vec![];
            let mut spawns = vec![];
            let mut deaths = vec![];

            value_tasks.push(tasks.spawn(async move {
                Self::update_values_chunk(&mut chunk, chunk_index, &rule,
                    &mut chunk_spawns, &mut spawns, &mut chunk_deaths, &mut deaths);
                (chunk, chunk_spawns, spawns, chunk_deaths, deaths)
            }));
        }

        // collect spawns & deaths.
        chunks = vec![];
        let mut chunk_spawns = vec![];
        let mut chunk_deaths = vec![];
        let mut spawns = vec![];
        let mut deaths = vec![];
        for task in value_tasks {
            let (chunk, in_spawns, out_spawns, in_deaths, out_deaths) = future::block_on(task);
            chunks.push(chunk);
            chunk_spawns.push(in_spawns);
            chunk_deaths.push(in_deaths);
            spawns.extend(out_spawns);
            deaths.extend(out_deaths);
        }


        // update neighbors parallel.
        let mut neighbour_tasks = vec![];
        for ((mut chunk, spawns), deaths) in chunks.into_iter().zip(chunk_spawns).zip(chunk_deaths)
        {
            let rule = rule.clone(); // shrug

            neighbour_tasks.push(tasks.spawn(async move {
                for offset in spawns {
                    Self::update_neighbors_chunk(&mut chunk, &rule, offset, true);
                }
                for offset in deaths {
                    Self::update_neighbors_chunk(&mut chunk, &rule, offset, false);
                }

                chunk
            }));
        }

        // collect chunks.
        chunks = vec![];
        for task in neighbour_tasks {
            let chunk = future::block_on(task);
            chunks.push(chunk);
        }


        // update neighbors serial.
        for index in spawns {
            self.update_neighbors(&mut chunks, rule, index, true);
        }
        for index in deaths {
            self.update_neighbors(&mut chunks, rule, index, false);
        }

        self.chunks.chunks = chunks;
    }


    // TEMP: move to sims.
    #[allow(dead_code)]
    pub fn validate(&self, rule: &Rule) {
        for index in 0..self.chunks.chunk_count*CHUNK_CELL_COUNT {
            let pos = self.chunks.index_to_pos(index);

            let mut neighbors = 0;
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let neighbour_pos = self.wrap(pos + *dir);

                let index  = self.chunks.pos_to_index(neighbour_pos);
                let chunk  = index_to_chunk_index(index);
                let offset = index_to_chunk_offset(index);
                if self.chunks.chunks[chunk].0[offset].value == rule.states {
                    neighbors += 1;
                }
            }

            let chunk  = index_to_chunk_index(index);
            let offset = index_to_chunk_offset(index);
            assert_eq!(neighbors, self.chunks.chunks[chunk].0[offset].neighbours);
        }
    }

    pub fn spawn_noise(&mut self, rule: &Rule) {
        let mut chunks = std::mem::take(&mut self.chunks.chunks);
        utils::make_some_noise_default(self.center(), |pos| {
            let index  = self.chunks.pos_to_index(self.wrap(pos));
            let chunk  = index_to_chunk_index(index);
            let offset = index_to_chunk_offset(index);
            let cell = &mut chunks[chunk].0[offset];
            if cell.is_dead() {
                cell.value = rule.states;
                self.update_neighbors(&mut chunks, rule, index, true);
            }
        });
        self.chunks.chunks = chunks;
    }
}


impl crate::cells::Sim for LeddooMultiThreaded {
    fn update(&mut self, input: &Input<KeyCode>, rule: &Rule, task_pool: &TaskPool) {
        if input.just_pressed(KeyCode::P) {
            self.spawn_noise(rule);
        }

        self.update(rule, task_pool);
    }

    fn render(&self, rule: &Rule, data: &mut Vec<InstanceData>) {
        for (chunk_index, chunk) in self.chunks.chunks.iter().enumerate() {
            for (index, cell) in chunk.0.iter().enumerate() {
                if cell.is_dead() {
                    continue;
                }

                let pos = self.chunks.index_to_pos(chunk_index*CHUNK_CELL_COUNT + index);
                data.push(InstanceData {
                    position: (pos - self.center()).as_vec3(),
                    scale: 1.0,
                    color: rule
                        .color_method
                        .color(
                            rule.states,
                            cell.value,
                            cell.neighbours,
                            utils::dist_to_center(pos, self.bounds()),
                        )
                        .as_rgba_f32(),
                });
            }
        }
    }

    fn reset(&mut self, _rule: &Rule) {
        *self = LeddooMultiThreaded::new();
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

