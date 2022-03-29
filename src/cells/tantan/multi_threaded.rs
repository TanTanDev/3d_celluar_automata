use std::{collections::HashMap, sync::Mutex};

use bevy::{
    math::{IVec3},
    tasks::{TaskPool, Task},
};
use futures_lite::future;
use std::sync::{Arc, RwLock};

use crate::{
    cell_renderer::{CellRenderer},
    rule::Rule,
    utils,
};

use super::CellState;

pub struct CellsMultithreaded {
    states: Arc<RwLock<HashMap<IVec3, CellState>>>,

    bounding_size: i32,

    // cached data used for calculating state
    neighbours: Arc<RwLock<HashMap<IVec3, u8>>>,
    changes: HashMap<IVec3, StateChange>,
    change_mask: HashMap<IVec3, bool>,

    change_results_cache: Vec<Arc<Mutex<Vec<(IVec3, StateChange)>>>>,
    neighbour_results_cache: Vec<Arc<Mutex<Vec<IVec3>>>>,

    position_thread_cache: Vec<Arc<Mutex<Vec<IVec3>>>>,
}

pub enum StateChange {
    Decay,
    Spawn {
        // metadata
        neighbours: u8,
    },
}

impl CellsMultithreaded {
    pub fn new() -> Self {
        CellsMultithreaded {
            states: Arc::new(RwLock::new(HashMap::new())),
            bounding_size: 0,
            neighbours: Arc::new(RwLock::new(HashMap::new())),
            changes: HashMap::new(),
            change_mask: HashMap::new(),
            position_thread_cache: Vec::new(),
            change_results_cache: Vec::new(),
            neighbour_results_cache: Vec::new(),
        }
    }

    pub fn tick(&mut self, rule: &Rule, task_pool: &TaskPool) {
        // neighbours
        {
            let neighbour_tasks = self.calculate_neighbours(rule, task_pool);
            for task in neighbour_tasks {
                future::block_on(task);
            }

            let mut neighbours = self.neighbours.write().unwrap();
            for neighbour_cache in self.neighbour_results_cache.iter() {
                let mut cache = neighbour_cache.lock().unwrap();
                for neighbour_pos in cache.drain(..) {
                    // reduces indentation :)
                    if !neighbours.contains_key(&neighbour_pos) {
                        neighbours.insert(neighbour_pos, 0);
                    }
                    let neighbour = neighbours.get_mut(&neighbour_pos).unwrap();
                    *neighbour += 1;

                    // udpate mask
                    match self.change_mask.get_mut(&neighbour_pos) {
                        Some(masked) => *masked = true,
                        None => {
                            self.change_mask.insert(neighbour_pos, true);
                        }
                    }
                }
            }
            // no neighbour is counted for current cell so add them to the mask
            self.states.read().unwrap().iter().for_each(|s| {
                match self.change_mask.get_mut(&s.0) {
                    Some(masked) => *masked = true,
                    None => {
                        self.change_mask.insert(*s.0, true);
                    }
                }
            });
            for cached_vec in self.position_thread_cache.iter() {
                cached_vec.lock().unwrap().clear();
            }
        }

        // changes
        {
            let change_tasks = self.calculate_changes(rule, task_pool);
            for task in change_tasks {
                future::block_on(task);
            }

            // join in the changes
            for change_cache in self.change_results_cache.iter() {
                let mut cache = change_cache.lock().unwrap();
                for (cell_pos, state_change) in cache.drain(..) {
                    self.changes.insert(cell_pos, state_change);
                }
            }

            self.apply_changes(rule);
            for cached_vec in self.position_thread_cache.iter() {
                cached_vec.lock().unwrap().clear();
            }
        }
    }

    pub fn calculate_neighbours(&mut self, rule: &Rule, task_pool: &TaskPool)
        -> Vec<Task<()>>
    {
        let states = self.states.read().unwrap();
        let job_count = task_pool.thread_num();
        let chunk_size = ((states.len() as f32 / job_count as f32).ceil() as usize).max(1);
        // i dynamically size the position_thread_cache in case the async_compute_task_pool threads increases
        while self.position_thread_cache.len() < job_count {
            self.position_thread_cache
                .push(Arc::new(Mutex::new(Vec::with_capacity(chunk_size))));
        }
        while self.neighbour_results_cache.len() < job_count {
            self.neighbour_results_cache
                .push(Arc::new(Mutex::new(Vec::with_capacity(chunk_size))));
        }

        states.iter().enumerate().for_each(|(i, p)| {
            let slice_index = i / chunk_size;
            let mut position_thread_target =
                self.position_thread_cache[slice_index].lock().unwrap();
            position_thread_target.push(*p.0);
        });
        drop(states);

        let mut tasks = vec![];
        for position_cache_index in 0..job_count {
            // prepare data needed for thread
            let state_rc_clone = self.states.clone();
            let rule_states = rule.states;
            let rule_bounding = self.bounding_size;
            let neighbour_method = rule.neighbour_method.clone();
            let position_cache = self.position_thread_cache[position_cache_index].clone();
            let result_cache = self.neighbour_results_cache[position_cache_index].clone();

            let neighbour_task = task_pool.spawn(async move {
                let position_cache = position_cache.lock().unwrap();
                let mut result_cache = result_cache.lock().unwrap();
                let states = state_rc_clone.read().unwrap();
                for cell_pos in position_cache.iter() {
                    if let Some(cell) = states.get(&cell_pos) {
                        // count as neighbour if new
                        if cell.value == rule_states {
                            // get neighbouring cells and increment
                            for dir in neighbour_method.get_neighbour_iter() {
                                let neighbour_pos = utils::wrap(*cell_pos + *dir, rule_bounding);
                                result_cache.push(neighbour_pos);
                            }
                        }
                    }
                }
            });
            tasks.push(neighbour_task);
        }

        tasks
    }

    pub fn calculate_changes(&mut self, rule: &Rule, task_pool: &TaskPool)
        -> Vec<Task<()>>
    {
        let job_count = task_pool.thread_num();
        let chunk_size =
            ((self.change_mask.len() as f32 / job_count as f32).ceil() as usize).max(1);
        // i dynamically size the position_thread_cache in case the async_compute_task_pool threads increases
        while self.position_thread_cache.len() < job_count {
            self.position_thread_cache
                .push(Arc::new(Mutex::new(Vec::with_capacity(chunk_size))));
        }
        while self.change_results_cache.len() < job_count {
            self.change_results_cache
                .push(Arc::new(Mutex::new(Vec::with_capacity(chunk_size))));
        }

        self.change_mask.iter().enumerate().for_each(|(i, p)| {
            let slice_index = i / chunk_size;
            let mut position_thread_target =
                self.position_thread_cache[slice_index].lock().unwrap();
            position_thread_target.push(*p.0);
        });

        let mut tasks = vec![];
        for position_cache_index in 0..job_count {
            // prepare data for thread
            let state_rc_clone = self.states.clone();
            let neighbours_rc_clone = self.neighbours.clone();
            let rule_survival_rule = rule.survival_rule.clone();
            let rule_birth_rule = rule.birth_rule.clone();
            let rule_start_state_value = rule.states;
            let rule_bounding = self.bounding_size;
            let position_cache = self.position_thread_cache[position_cache_index].clone();
            let change_results_cache = self.change_results_cache[position_cache_index].clone();

            let changes_task = task_pool.spawn(async move {
                let position_cache = position_cache.lock().unwrap();
                let mut change_results_cache = change_results_cache.lock().unwrap();
                let states = state_rc_clone.read().unwrap();
                let neighbours = neighbours_rc_clone.read().unwrap();
                for cell_pos in position_cache.iter() {
                    let neighbours = match neighbours.get(cell_pos) {
                        Some(n) => *n,
                        None => 0,
                    };
                    match states.get(cell_pos) {
                        Some(cell) => {
                            if !(rule_survival_rule.in_range_incorrect(neighbours)
                                && cell.value == rule_start_state_value)
                            {
                                change_results_cache.push((*cell_pos, StateChange::Decay));
                            }
                        }
                        None => {
                            // check if should spawn
                            if rule_birth_rule.in_range_incorrect(neighbours) {
                                if utils::is_in_bounds(*cell_pos, rule_bounding) {
                                    change_results_cache
                                        .push((*cell_pos, StateChange::Spawn { neighbours }));
                                }
                            }
                        }
                    }
                }
            });
            tasks.push(changes_task);
        }

        tasks
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
                StateChange::Spawn { neighbours } => {
                    // TODODDKJ
                    states.insert(
                        *cell_pos,
                        CellState::new(
                            rule.states,
                            *neighbours,
                        ),
                    );
                }
            }
        }

        // remove dead
        states.retain(|_, c| c.value > 0);

        // ALL calculations are done, reset cached data
        self.changes.clear();
        // self.change_mask.clear();
        self.change_mask.iter_mut().for_each(|m| *m.1 = false);
        self.neighbours.write().unwrap().clear();
    }
}


impl crate::cells::Sim for CellsMultithreaded {
    fn update(&mut self, rule: &Rule, task_pool: &TaskPool) {
        self.tick(&rule, &task_pool);
    }

    fn render(&self, renderer: &mut CellRenderer) {
        renderer.clear();
        for cell in self.states.read().unwrap().iter() {
            renderer.set_pos(*cell.0, cell.1.value, cell.1.neighbours);
        }
    }

    fn spawn_noise(&mut self, rule: &Rule) {
        let states = &mut self.states.write().unwrap();
        utils::make_some_noise_default(utils::center(self.bounding_size), |pos| {
            states.insert(pos, CellState::new(rule.states, 0));
        });
    }

    fn cell_count(&self) -> usize {
        self.states.read().unwrap().len()
    }
    
    fn bounds(&self) -> i32 {
        self.bounding_size
    }

    fn set_bounds(&mut self, new_bounds: i32) -> i32 {
        if new_bounds != self.bounding_size {
            *self = CellsMultithreaded::new();
        }
        self.bounding_size = new_bounds;
        new_bounds
    }
}
