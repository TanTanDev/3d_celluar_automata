use std::collections::HashMap;

use bevy::{
    input::Input,
    math::{ivec3, vec3, IVec3},
    prelude::{Color, KeyCode, Plugin, Query, Res, ResMut, info_span},
    tasks::{AsyncComputeTaskPool, Task}
};
use std::sync::{Arc, RwLock};
use futures_lite::future;

use crate::{
    cell_renderer::{InstanceData, InstanceMaterialData},
    neighbours::MOOSE_NEIGHBOURS,
    rule::Rule,
    utils::{self, keep_in_bounds},
    State,
};

struct CellsMultithreaded {
    states: Arc<RwLock<HashMap<IVec3, State>>>,

    // cached datta used for calculating state
    neighbours: Arc<RwLock<HashMap<IVec3, u8>>>,
    changes: HashMap<IVec3, StateChange>,
    change_mask: HashMap<IVec3, bool>,

    neighbour_jobs: Vec<Option<Task<Vec<IVec3>>>>,
    change_jobs: Vec<Option<Task<Vec<(IVec3, StateChange)>>>>,
    process_step: ProcessStep,
    accumulated_neighbour_writes: Vec<IVec3>,

    // the instance buffer data
    instance_material_data: Option<Vec<InstanceData>>,
}

pub enum StateChange {
    Decay,
    Spawn,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProcessStep {
    Ready,
    CalculateNeighbours,
    AwaitNeighbours,
    CalculateChanges,
    AwaitChanges,
    // the final step is to apply the changes from the data
    // instead of having a seperate state for doing that, it's done it the AwaitChanges state 
    // to avoid having to wait 1 frame to get into that state
}

impl ProcessStep {
    pub fn advance_to_next_step(&mut self) {
        match self {
            ProcessStep::Ready => *self = ProcessStep::CalculateNeighbours,
            ProcessStep::CalculateNeighbours => *self = ProcessStep::AwaitNeighbours,
            ProcessStep::AwaitNeighbours => *self = ProcessStep::CalculateChanges,
            ProcessStep::CalculateChanges => *self = ProcessStep::AwaitChanges,
            ProcessStep::AwaitChanges => *self = ProcessStep::Ready,
        }
    }
}

impl CellsMultithreaded {
    pub fn new(rule: &Rule) -> Self {
        let s = CellsMultithreaded {
            states: Arc::new(RwLock::new(HashMap::new())),
            neighbours: Arc::new(RwLock::new(HashMap::new())),
            changes: HashMap::new(),
            change_mask: HashMap::new(),
            neighbour_jobs: Vec::new(),
            change_jobs: Vec::new(),
            process_step: ProcessStep::CalculateNeighbours,
            instance_material_data: None,
            accumulated_neighbour_writes: Vec::new(),
        };
        utils::spawn_noise(&mut s.states.write().unwrap(), rule);
        s
    }

    pub fn ready(&mut self) {
        if self.process_step == ProcessStep::Ready {
            self.process_step.advance_to_next_step();
        }
    }

    pub fn is_busy(&mut self) -> bool {
        self.process_step != ProcessStep::Ready
    }

    pub fn tick(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        let advance = match self.process_step {
            ProcessStep::Ready => false,
            ProcessStep::CalculateNeighbours => {
                self.calculate_neighbours(rule, task_pool);
                true
            },
            ProcessStep::AwaitNeighbours => {
                for job in self.neighbour_jobs.iter_mut() {
                    let mut task = job.take().unwrap();
                    match future::block_on(future::poll_once(&mut task)) {
                        Some(mut results) => {
                            self.accumulated_neighbour_writes.append(&mut results);
                        },
                        // failed to retrieve data continue
                        None => *job = Some(task),
                    }
                }
                // remove completed tasks
                self.neighbour_jobs.retain(|job| job.is_some());

                let is_done = self.neighbour_jobs.is_empty();
                if is_done {
                    let mut neighbours = self.neighbours.write().unwrap();
                    for neighbour_pos in self.accumulated_neighbour_writes.iter() {
                        // reduces indentation :)
                        if !neighbours.contains_key(&neighbour_pos) {
                            neighbours.insert(*neighbour_pos, 0);
                        }
                        let neighbour = neighbours.get_mut(&neighbour_pos).unwrap();
                        *neighbour += 1;

                        // udpate mask
                        match self.change_mask.get_mut(&neighbour_pos) {
                            Some(masked) => *masked = true,
                            None => { self.change_mask.insert(*neighbour_pos, true); },
                        }
                    }
                    // no neighbour is counted for current cell so add them to the mask
                    self.states.read().unwrap().iter().for_each(|s| {
                        match self.change_mask.get_mut(&s.0) {
                            Some(masked) => *masked = true,
                            None => { self.change_mask.insert(*s.0, true); },
                        }
                    });
                    self.accumulated_neighbour_writes.clear();
                }
                is_done
            },
            ProcessStep::CalculateChanges => {
                //println!("num cells: {}", self.states.read().unwrap().len());
                self.calculate_changes(rule, task_pool);
                true
            },
            ProcessStep::AwaitChanges => {
                for job in self.change_jobs.iter_mut() {
                    let mut task = job.take().unwrap();
                    match future::block_on(future::poll_once(&mut task)) {
                        Some(state_changes) => {
                            //println!("usual state changes: {:?}", state_changes.len());
                            for (cell_pos, state_change) in state_changes.into_iter() {
                                self.changes.insert(cell_pos, state_change);
                            }
                        },
                        // failed to retrieve data continue
                        None => *job = Some(task),
                    }
                }
                // remove completed tasks
                self.change_jobs.retain(|job| job.is_some());
                let done = self.change_jobs.is_empty();
                if done {
                    self.apply_changes(rule);
                }
                done
            },
        };
        if advance {
            self.process_step.advance_to_next_step();
        }
    }

    pub fn calculate_neighbours(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        //let (x_range, y_range, z_range) = rule.get_bounding_ranges();
        let states = self.states.read().unwrap();
        let mut positions: Vec<IVec3> = Vec::with_capacity(states.len());
        states.iter().for_each(|k|positions.push(*k.0));
        // drop the read of states
        drop(states);
        let job_count = task_pool.thread_num() * 2;
        let chunk_size = (positions.len() / job_count).max(1);
        let sliced_positions: Vec<Vec<IVec3>> = positions.chunks(chunk_size).map(|v|v.into()).collect();

        for pos_slice in sliced_positions.into_iter() {
            // prepare data needed for thread
            let state_rc_clone = self.states.clone();
            let rule_states = rule.states;
            let rule_bounding = rule.bounding;

            let neighbour_task = task_pool.spawn(async move {
                let mut results: Vec<IVec3> = vec![];
                let states = state_rc_clone.read().unwrap();
                for cell_pos in pos_slice.into_iter() {
                    if let Some(cell) = states.get(&cell_pos) {
                            // count as neighbour if new
                        if cell.value == rule_states {
                            // get neighbouring cells and increment
                            for dir in MOOSE_NEIGHBOURS.iter() {
                                let mut neighbour_pos = cell_pos + *dir;
                                keep_in_bounds(rule_bounding, &mut neighbour_pos);
                                results.push(neighbour_pos);
                            }
                        }
                    }
                }
                results
            });
            self.neighbour_jobs.push(Some(neighbour_task));
        }
    }

    pub fn calculate_changes(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        let mut positions: Vec<IVec3> = Vec::with_capacity(self.change_mask.len());
        self.change_mask.iter().filter(|k| *k.1).for_each(|k|positions.push(*k.0));
        let job_count = task_pool.thread_num() * 2;
        let chunk_size = (positions.len() / job_count).max(1);
        let sliced_mask: Vec<Vec<IVec3>> = positions.chunks(chunk_size).map(|v|v.into()).collect();
        for slice_mask in sliced_mask.into_iter() {
            // prepare data for thread
            let state_rc_clone = self.states.clone();
            let neighbours_rc_clone = self.neighbours.clone();
            let rule_survival_rule = rule.survival_rule.clone();
            let rule_birth_rule = rule.birth_rule.clone();
            let rule_start_state_value = rule.start_state_value;
            let rule_bounding = rule.bounding;
            let changes_task = task_pool.spawn(async move {
                let mut changes = Vec::new();
                let states = state_rc_clone.read().unwrap();
                let neighbours = neighbours_rc_clone.read().unwrap();
                for cell_pos in slice_mask {
                    let neighbours = match neighbours.get(&cell_pos) {
                        Some(n) => *n,
                        None => 0,
                    };
                    match states.get(&cell_pos) {
                        Some(cell) => {
                            if !(rule_survival_rule.in_range(neighbours)
                                && cell.value == rule_start_state_value)
                            {
                                changes.push((cell_pos, StateChange::Decay));
                            }
                        }
                        None => {
                            // check if should spawn
                            if rule_birth_rule.in_range(neighbours) {
                                if cell_pos.x >= -rule_bounding
                                    && cell_pos.x <= rule_bounding
                                    && cell_pos.y >= -rule_bounding
                                    && cell_pos.y <= rule_bounding
                                    && cell_pos.z >= -rule_bounding
                                    && cell_pos.z <= rule_bounding
                                {
                                    changes.push((cell_pos, StateChange::Spawn));
                                }
                            }
                        }
                    }
                }
                changes
            });
            self.change_jobs.push(Some(changes_task));
        }
    }

    pub fn apply_changes(&mut self, rule: &Rule) {
        let mut states = self.states.write().unwrap();
        // apply new spawns
        for (cell_pos, state_change) in self.changes.iter() {
            match state_change {
                StateChange::Decay => {
                    let mut cell = states.get_mut(cell_pos).unwrap();
                    // DECAY BY 1 value
                    let value = cell.value as i32 - 1;
                    let value = i32::min(value, rule.states as i32);
                    cell.value = value as u8;
                }
                StateChange::Spawn => {
                    states
                        .insert(*cell_pos, State::new(rule.start_state_value));
                }
            }
        }

        // remove dead
        states.retain(|_, c| c.value > 0);

        // update instance buffer
        let mut instance_data = Vec::new();
        for cell in states.iter() {
            let pos = cell.0;
            instance_data.push(InstanceData {
                position: vec3(pos.x as f32, pos.y as f32, pos.z as f32),
                scale: 1.0,
                color: Color::rgba(cell.1.value as f32 / rule.states as f32, 0.0, 0.0, 1.0)
                    .as_rgba_f32(),
            });
        }
        self.instance_material_data = Some(instance_data);


        // ALL calculations are done, reset cached data
        self.changes.clear();
        // self.change_mask.clear();
        self.change_mask.iter_mut().for_each(|m|*m.1 = false);
        self.neighbours.write().unwrap().clear();
    }
}

fn tick_cell(
    rule: Res<Rule>,
    mut cells: ResMut<CellsMultithreaded>,
    keyboard_input: Res<Input<KeyCode>>,
    task_pool: Res<AsyncComputeTaskPool>,
) {
    cells.tick(&rule, task_pool);
    if keyboard_input.pressed(KeyCode::E) {
        cells.ready();
        return;
    }
}

fn spawn_noise(
    rule: Res<Rule>,
    mut cells: ResMut<CellsMultithreaded>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if !keyboard_input.pressed(KeyCode::P) {
        return;
    }
    if cells.is_busy() {
        return;
    }
    utils::spawn_noise(&mut cells.states.write().unwrap(), &rule);
    cells.ready();
}

pub struct CellsMultithreadedPlugin;
impl Plugin for CellsMultithreadedPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        let rule = app.world.get_resource::<Rule>().unwrap();
        let cells_singlethreaded = CellsMultithreaded::new(&rule);
        app.insert_resource(cells_singlethreaded)
            .add_system(prepare_cell_data)
            .add_system(spawn_noise)
            .add_system(tick_cell);
    }
}

fn prepare_cell_data(
    mut cells: ResMut<CellsMultithreaded>,
    mut query: Query<&mut InstanceMaterialData>,
) {
    // take the first
    if let Some(mut instance_material_data) = cells.instance_material_data.take() {
        let mut instance_data = query.iter_mut().next().unwrap();
        instance_data.0.clear();
        instance_data.0.append(&mut instance_material_data);
    }
}
