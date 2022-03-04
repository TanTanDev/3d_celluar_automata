use std::collections::HashMap;

use bevy::{
    input::Input,
    math::{ivec3, vec3, IVec3},
    prelude::{Color, KeyCode, Plugin, Query, Res, ResMut},
};

use crate::{
    cell_renderer::{InstanceData, InstanceMaterialData},
    neighbours::MOOSE_NEIGHBOURS,
    rule::Rule,
    utils::{self, keep_in_bounds},
    State,
};

struct CellsSinglethreaded {
    states: HashMap<IVec3, State>,
    // cached datta used for calculating state
    neighbours: HashMap<IVec3, u8>,
    changes: HashMap<IVec3, i32>,
    spawn: Vec<IVec3>,
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
        let (x_range, y_range, z_range) = rule.get_bounding_ranges();

        for z in z_range.clone() {
            for y in y_range.clone() {
                for x in x_range.clone() {
                    let cell_pos = ivec3(x, y, z);
                    if let Some(cell) = self.states.get(&cell_pos) {
                        // count as neighbour if new
                        if cell.value == rule.states {
                            // get neighbouring cells and increment
                            for dir in MOOSE_NEIGHBOURS.iter() {
                                let mut neighbour_pos = cell_pos + *dir;
                                keep_in_bounds(rule.bounding, &mut neighbour_pos);
                                if !self.neighbours.contains_key(&neighbour_pos) {
                                    self.neighbours.insert(neighbour_pos, 0);
                                }
                                let neighbour = self.neighbours.get_mut(&neighbour_pos).unwrap();
                                *neighbour += 1;
                            }
                        }
                    }
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
                            if !(rule.survival_rule.in_range(neighbours)
                                && cell.value == rule.start_state_value)
                            {
                                self.changes.insert(cell_pos, -1i32);
                            }
                        }
                        None => {
                            // check if should spawn
                            if rule.birth_rule.in_range(neighbours) {
                                if cell_pos.x >= -rule.bounding
                                    && cell_pos.x <= rule.bounding
                                    && cell_pos.y >= -rule.bounding
                                    && cell_pos.y <= rule.bounding
                                    && cell_pos.z >= -rule.bounding
                                    && cell_pos.z <= rule.bounding
                                {
                                    self.spawn.push(cell_pos);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn apply_changes(&mut self, rule: &Rule) {
        // apply new spawns
        for cell_pos in self.spawn.iter() {
            self.states
                .insert(*cell_pos, State::new(rule.start_state_value));
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

fn tick_cell(
    rule: Res<Rule>,
    mut cells: ResMut<CellsSinglethreaded>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if !keyboard_input.pressed(KeyCode::E) {
        return;
    }
    cells.tick(&rule);
}

fn spawn_noise(
    rule: Res<Rule>,
    mut cells: ResMut<CellsSinglethreaded>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if !keyboard_input.just_pressed(KeyCode::P) {
        return;
    }
    utils::spawn_noise(&mut cells.states, &rule);
}

pub struct CellsSinglethreadedPlugin;
impl Plugin for CellsSinglethreadedPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        let cells_singlethreaded = CellsSinglethreaded::new();
        app.insert_resource(cells_singlethreaded)
            .add_system(prepare_cell_data)
            .add_system(spawn_noise)
            .add_system(tick_cell);
    }
}

fn prepare_cell_data(
    rule: Res<Rule>,
    cells: Res<CellsSinglethreaded>,
    mut query: Query<&mut InstanceMaterialData>,
) {
    // take the first
    let mut instance_data = query.iter_mut().next().unwrap();
    instance_data.0.clear();
    for cell in cells.states.iter() {
        let pos = cell.0;
        instance_data.0.push(InstanceData {
            position: vec3(pos.x as f32, pos.y as f32, pos.z as f32),
            scale: 1.0,
            color: Color::rgba(cell.1.value as f32 / rule.states as f32, 0.0, 0.0, 1.0)
                .as_rgba_f32(),
        });
    }
}