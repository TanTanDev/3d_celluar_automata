use std::collections::HashMap;

use bevy::{
    math::{ivec3, vec3, IVec3},
    prelude::{Input, KeyCode},
    tasks::TaskPool,
};

use crate::{
    cell_renderer::{InstanceData},
    rule::Rule,
    utils,
};

use super::CellState;

pub struct CellsSinglethreaded {
    states: HashMap<IVec3, CellState>,
    // cached datta used for calculating state
    neighbours: HashMap<IVec3, u8>,
    changes: HashMap<IVec3, i32>,
    spawn: Vec<(IVec3, u8)>, // neighbours
}

impl CellsSinglethreaded {
    pub fn new() -> Self {
        CellsSinglethreaded {
            states: HashMap::new(),
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
                    let neighbour_pos = utils::wrap(*cell_pos + *dir, rule.bounding_size);
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
        let (x_range, y_range, z_range) = rule.get_bounding_ranges();
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
                    utils::dist_to_center(*cell_pos, rule),
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
    fn update(&mut self, input: &Input<KeyCode>, rule: &Rule, _task_pool: &TaskPool) {
        if input.just_pressed(KeyCode::P) {
            utils::make_some_noise_default(rule.center(), |pos| {
                let dist = utils::dist_to_center(pos, &rule);
                self.states.insert(pos, CellState::new(rule.states, 0, dist));
            });
        }

        self.tick(rule);
    }

    fn render(&self, rule: &Rule, data: &mut Vec<InstanceData>) {
        for cell in self.states.iter() {
            let pos = *cell.0 - rule.center();
            data.push(InstanceData {
                position: vec3(pos.x as f32, pos.y as f32, pos.z as f32),
                scale: 1.0,
                color: rule
                    .color_method
                    .color(
                        rule.states,
                        cell.1.value,
                        cell.1.neighbours,
                        cell.1.dist_to_center,
                    )
                    .as_rgba_f32(),
            });
        }
    }

    fn reset(&mut self, _rule: &Rule) {
        *self = CellsSinglethreaded::new();
    }

    fn cell_count(&self) -> usize {
        self.states.len()
    }
}

