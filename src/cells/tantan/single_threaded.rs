use std::collections::HashMap;

use bevy::{
    math::{ivec3, IVec3},
    tasks::TaskPool,
};

use crate::{
    cell_renderer::{CellRenderer},
    rule::Rule,
    utils,
};

use super::CellState;

pub struct CellsSinglethreaded {
    states: HashMap<IVec3, CellState>,
    bounding_size: i32,
    // cached datta used for calculating state
    neighbours: HashMap<IVec3, u8>,
    changes: HashMap<IVec3, i32>,
    spawn: Vec<(IVec3, u8)>, // neighbours
}

impl CellsSinglethreaded {
    pub fn new() -> Self {
        CellsSinglethreaded {
            states: HashMap::new(),
            bounding_size: 0,
            neighbours: HashMap::new(),
            changes: HashMap::new(),
            spawn: Vec::new(),
        }
    }

    pub fn tick(&mut self, rule: &Rule) {
        self.calculate_neighbours(rule);
        self.calculate_changes(rule);
        self.apply_changes(rule);
    }

    pub fn calculate_neighbours(&mut self, rule: &Rule) {
        for (cell_pos, cell) in self.states.iter() {
            // count as neighbour if new
            if cell.value == rule.states {
                // get neighbouring cells and increment
                for dir in rule.neighbour_method.get_neighbour_iter() {
                    let neighbour_pos = utils::wrap(*cell_pos + *dir, self.bounding_size);
                    if !self.neighbours.contains_key(&neighbour_pos) {
                        self.neighbours.insert(neighbour_pos, 0);
                    }
                    let neighbour = self.neighbours.get_mut(&neighbour_pos).unwrap();
                    *neighbour += 1;
                }
            }
        }
    }

    pub fn calculate_changes(&mut self, rule: &Rule) {
        let (x_range, y_range, z_range) = utils::get_bounding_ranges(self.bounding_size);
        for z in z_range {
            for y in y_range.clone() {
                for x in x_range.clone() {
                    let cell_pos = ivec3(x, y, z);
                    let neighbours = match self.neighbours.get(&cell_pos) {
                        Some(n) => *n,
                        None => 0,
                    };
                    match self.states.get(&cell_pos) {
                        Some(cell) => {
                            if !(rule.survival_rule.in_range_incorrect(neighbours)
                                && cell.value == rule.states)
                            {
                                self.changes.insert(cell_pos, -1i32);
                            }
                        }
                        None => {
                            // check if should spawn
                            if rule.birth_rule.in_range_incorrect(neighbours) {
                                // cell_pos is in bounds, because we iterate over the bounds.
                                self.spawn.push((cell_pos, neighbours));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn apply_changes(&mut self, rule: &Rule) {
        // apply new spawns
        for (cell_pos, neighbours) in self.spawn.iter() {
            self.states.insert(
                *cell_pos,
                CellState::new(
                    rule.states,
                    *neighbours,
                ),
            );
        }
        // apply state changes
        for changes in self.changes.iter() {
            let mut cell = self.states.get_mut(changes.0).unwrap();
            let value = cell.value as i32 + changes.1;
            let value = i32::min(value, rule.states as i32);
            cell.value = value as u8;
        }
        // remove dead
        self.states.retain(|_, c| c.value > 0);

        // ALL calculations are done, reset cached data
        self.spawn.clear();
        self.changes.clear();
        self.neighbours.clear();
    }
}


impl crate::cells::Sim for CellsSinglethreaded {
    fn update(&mut self, rule: &Rule, _task_pool: &TaskPool) {
        self.tick(rule);
    }

    fn render(&self, renderer: &mut CellRenderer) {
        renderer.clear();
        for cell in self.states.iter() {
            renderer.set_pos(*cell.0, cell.1.value, cell.1.neighbours);
        }
    }

    fn spawn_noise(&mut self, rule: &Rule) {
        utils::make_some_noise_default(utils::center(self.bounding_size), |pos| {
            self.states.insert(pos, CellState::new(rule.states, 0));
        });
    }

    fn cell_count(&self) -> usize {
        self.states.len()
    }

    fn bounds(&self) -> i32 {
        self.bounding_size
    }

    fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        if new_bounds != self.bounding_size {
            *self = CellsSinglethreaded::new();
        }
        self.bounding_size = new_bounds;
        new_bounds
    }
}

