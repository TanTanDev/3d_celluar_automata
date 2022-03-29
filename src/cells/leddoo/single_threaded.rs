/*
    how it works:
        - cells are stored as a 3d array.
        - see utils.rs for index <-> position conversion.
        - neighbor counts are stored persistently.
            - on every change, the neighbors need to be updated (eg: spawn_noise).
            - each tick updates the cells, and then only has to update the
            neighbors of cells that actually changed.
        - on my machine, this is ~20x faster than tantan's single threaded impl.
*/

use bevy::{
    math::{IVec3},
    tasks::TaskPool,
};

use crate::{
    cell_renderer::{CellRenderer},
    rule::Rule,
    utils,
};


#[derive(Clone, Copy)]
struct Cell {
    value: u8,
    neighbors: u8,
}

impl Cell {
    fn is_dead(self) -> bool {
        self.value == 0
    }
}


pub struct LeddooSingleThreaded {
    cells: Vec<Cell>,
    bounds: i32,
}

impl LeddooSingleThreaded {
    pub fn new() -> Self {
        LeddooSingleThreaded {
            cells: vec![],
            bounds: 0,
        }
    }

    pub fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        if new_bounds != self.bounds {
            self.cells.clear();
            self.cells.resize(
                (new_bounds*new_bounds*new_bounds) as usize,
                Cell { value: 0, neighbors: 0 });
            self.bounds = new_bounds;
        }
        self.bounds
    }

    pub fn cell_count(&self) -> usize {
        let mut result = 0;
        for cell in &self.cells {
            if !cell.is_dead() {
                result += 1;
            }
        }
        result
    }


    fn index_to_pos(&self, index: usize) -> IVec3 {
        utils::index_to_pos(index, self.bounds)
    }

    fn pos_to_index(&self, vec: IVec3) -> usize {
        utils::pos_to_index(vec, self.bounds)
    }

    pub fn wrap(&self, pos: IVec3) -> IVec3 {
        utils::wrap(pos, self.bounds)
    }


    fn update_neighbors(&mut self, rule: &Rule, index: usize, inc: bool) {
        let pos = self.index_to_pos(index);
        for dir in rule.neighbour_method.get_neighbour_iter() {
            let neighbor_pos = self.wrap(pos + *dir);

            let index = self.pos_to_index(neighbor_pos);
            if inc {
                self.cells[index].neighbors += 1;
            }
            else {
                self.cells[index].neighbors -= 1;
            }
        }
    }

    pub fn update(&mut self, rule: &Rule) {
        // TODO: detect neighbor rule change.

        let mut spawns = vec![];
        let mut deaths = vec![];

        // update values.
        for (index, cell) in self.cells.iter_mut().enumerate() {
            if cell.is_dead() {
                if rule.birth_rule.in_range(cell.neighbors) {
                    cell.value = rule.states;
                    spawns.push(index);
                }
            }
            else {
                if cell.value < rule.states || !rule.survival_rule.in_range(cell.neighbors) {
                    if cell.value == rule.states {
                        deaths.push(index);
                    }
                    cell.value -= 1;
                }
            }
        }

        // update neighbors.
        for index in spawns {
            self.update_neighbors(rule, index, true);
        }
        for index in deaths {
            self.update_neighbors(rule, index, false);
        }
    }

    // TEMP: move to sims.
    #[allow(dead_code)]
    pub fn validate(&self, rule: &Rule) {
        for index in 0..self.cells.len() {
            let pos = self.index_to_pos(index);

            let mut neighbors = 0;
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let neighbor_pos = self.wrap(pos + *dir);

                let index = self.pos_to_index(neighbor_pos);
                if self.cells[index].value == rule.states {
                    neighbors += 1;
                }
            }

            assert_eq!(neighbors, self.cells[index].neighbors);
        }
    }

    pub fn spawn_noise(&mut self, rule: &Rule) {
        utils::make_some_noise_default(utils::center(self.bounds), |pos| {
            let index = self.pos_to_index(self.wrap(pos));
            if self.cells[index].is_dead() {
                self.cells[index].value = rule.states;
                self.update_neighbors(rule, index, true);
            }
        });
    }
}


impl crate::cells::Sim for LeddooSingleThreaded {
    fn update(&mut self, rule: &Rule, _task_pool: &TaskPool) {
        self.update(rule);
    }

    fn render(&self, renderer: &mut CellRenderer) {
        for (index, cell) in self.cells.iter().enumerate() {
            renderer.set(index, cell.value, cell.neighbors);
        }
    }

    fn spawn_noise(&mut self, rule: &Rule) {
        self.spawn_noise(rule);
    }

    fn cell_count(&self) -> usize {
        self.cell_count()
    }

    fn bounds(&self) -> i32 {
        self.bounds
    }

    fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        self.set_bounds(new_bounds)
    }
}
