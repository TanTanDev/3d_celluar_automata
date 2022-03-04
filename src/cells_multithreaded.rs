use std::collections::HashMap;

use bevy::{
    input::Input,
    math::{ivec3, vec3, IVec3},
    prelude::{Color, KeyCode, Plugin, Query, Res, ResMut},
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

    neighbour_jobs: Vec<Option<Task<Vec<IVec3>>>>,
    change_jobs: Vec<Option<Task<Vec<(IVec3, StateChange)>>>>,
    process_step: ProcessStep,

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
            neighbour_jobs: Vec::new(),
            change_jobs: Vec::new(),
            process_step: ProcessStep::CalculateNeighbours,
            instance_material_data: None,
        };
        utils::spawn_noise(&mut s.states.write().unwrap(), rule);
        s
    }

    pub fn ready(&mut self) {
        if self.process_step == ProcessStep::Ready {
            self.process_step.advance_to_next_step();
        }
    }

    pub fn tick(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        println!("tick");
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
                        Some(results) => {
                            let mut neighbours = self.neighbours.write().unwrap();
                            for neighbour_pos in results.into_iter() {
                                if !neighbours.contains_key(&neighbour_pos) {
                                    neighbours.insert(neighbour_pos, 0);
                                }
                                let neighbour = neighbours.get_mut(&neighbour_pos).unwrap();
                                *neighbour += 1;
                            }
                        },
                        // failed to retrieve data continue
                        None => *job = Some(task),
                    }
                }
                // remove completed tasks
                self.neighbour_jobs.retain(|job| job.is_some());

                // no jobs -> advance process step, jobs,left stay at this step
                self.neighbour_jobs.is_empty()
            },
            ProcessStep::CalculateChanges => {
                self.calculate_changes(rule, task_pool);
                true
            },
            ProcessStep::AwaitChanges => {
                for job in self.change_jobs.iter_mut() {
                    let mut task = job.take().unwrap();
                    match future::block_on(future::poll_once(&mut task)) {
                        Some(state_changes) => {
                            let mut states = self.states.write().unwrap();
                            for (cell_pos, state_change) in state_changes.into_iter() {
                                match state_change {
                                    StateChange::Decay => {
                                        let mut cell = states.get_mut(&cell_pos).unwrap();
                                        cell.value -= 1;
                                    },
                                    StateChange::Spawn => {
                                        states.insert(cell_pos, State::new(rule.start_state_value));
                                    },
                                }
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
            println!("advanced to step: {:?}", self.process_step);
        }
    }

    pub fn calculate_neighbours(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        let (x_range, y_range, z_range) = rule.get_bounding_ranges();
        for z in z_range.clone() {
            for y in y_range.clone() {
                for x in x_range.clone() {

                    // prepare data needed for thread
                    let state_rc_clone = self.states.clone();
                    let rule_states = rule.states;
                    let rule_bounding = rule.bounding;
                    let cell_pos = ivec3(x, y, z);

                    let neighbour_task = task_pool.spawn(async move {
                        let states = state_rc_clone.read().unwrap();

                        let mut results: Vec<IVec3> = vec![];
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
                        results
                    });
                    self.neighbour_jobs.push(Some(neighbour_task));
                }
            }
        }
    }

    pub fn calculate_changes(&mut self, rule: &Rule, task_pool: Res<AsyncComputeTaskPool>) {
        let (x_range, y_range, z_range) = rule.get_bounding_ranges();
        for z in z_range {
            for y in y_range.clone() {
                for x in x_range.clone() {

                    // prepare data for thread
                    let cell_pos = ivec3(x, y, z);
                    let state_rc_clone = self.states.clone();
                    let neighbours_rc_clone = self.neighbours.clone();
                    let rule_survival_rule = rule.survival_rule.clone();
                    let rule_birth_rule = rule.birth_rule.clone();
                    let rule_start_state_value = rule.start_state_value;
                    let rule_bounding = rule.bounding;

                    let changes_task = task_pool.spawn(async move {
                        let states = state_rc_clone.read().unwrap();
                        let mut changes = Vec::new();
                        let neighbours = match neighbours_rc_clone.read().unwrap().get(&cell_pos) {
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
                        changes
                    });
                    self.change_jobs.push(Some(changes_task));
                }
            }
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
    if !keyboard_input.just_pressed(KeyCode::P) {
        return;
    }
    //utils::spawn_noise(&mut cells.states, &rule);
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
        println!("wrote to instance data");
        let mut instance_data = query.iter_mut().next().unwrap();
        instance_data.0.clear();
        instance_data.0.append(&mut instance_material_data);
    }
}
