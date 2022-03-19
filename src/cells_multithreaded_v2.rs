use std::hash::Hash;
use std::ops::{Index, IndexMut};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use bevy::tasks::{ComputeTaskPool, ParallelSlice, TaskPool, ParallelSliceMut};
use bevy::{
    input::Input,
    math::{ivec3, vec3, IVec3},
    prelude::*,
};
use rand::Rng;

use crate::rotating_camera::UpdateEvent;
use crate::{
    cell_renderer::{InstanceData, InstanceMaterialData},
    rule::Rule,
    utils::{self, keep_in_bounds},
};

struct Cells {
    /// Cache of [`Rule::bounding_size`]
    bounding_size: i32,

    /// length of one side of the 3d array
    side_length: usize,

    /// 3D arrary of (value, neighbours)
    data: Vec<(u8, AtomicU8)>,
}

impl Cells {
    fn zeros(rule: &Rule) -> Self {

        let side_length: usize = (rule.bounding_size * 2 + 1).try_into().unwrap();

        let vec = (0..side_length.pow(3))
            .map(|_| (0.into(), 0.into()))
            .collect();

        Self {
            bounding_size: rule.bounding_size,
            side_length,
            data: vec,
        }
    }

    fn len(&self) -> usize {
        self.side_length.pow(3)
    }

    fn get(&self, pos: &IVec3) -> &(u8, AtomicU8) {
        let index = self.to_index(pos);
        &self[index]
    }

    fn get_mut(&mut self, pos: &IVec3) -> &mut (u8, AtomicU8) {
        let index = self.to_index(pos);
        &mut self[index]
    }

    fn to_index(&self, pos: &IVec3) -> usize {
        let i: usize = (pos.x + self.bounding_size).try_into().unwrap();
        let j: usize = (pos.y + self.bounding_size).try_into().unwrap();
        let k: usize = (pos.z + self.bounding_size).try_into().unwrap();

        i * self.side_length * self.side_length + j * self.side_length + k
    }

    fn to_pos(&self, mut index: usize) -> IVec3 {

        let i = index / (self.side_length * self.side_length);
        index %= self.side_length * self.side_length;

        let j = index / (self.side_length);
        index %= self.side_length;

        let k = index;

        
        (
            i as i32 - self.bounding_size,
            j as i32 - self.bounding_size,
            k as i32 - self.bounding_size,
        )
        .into()
    }

    fn spawn_noise(
        &mut self,
        // _task_pool: &TaskPool, // TODO: parallelize?
        rule: &Rule
    ) {
        let timer = Instant::now();
        
        // Different version of `utils::spawn_noise`
        let mut rand = rand::thread_rng();
        let spawn_size = 6;
        (0..12 * 12 * 12).for_each(|_i| {
            let pos = ivec3(
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
            );
            let (value, _) = self.get_mut(&pos);
            *value = rule.states;
        });

        info!(
            "spawn_noise: {:.3} ms",
            timer.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn increment_neighbor(&self, pos: &IVec3) {
        self.get(pos).1.fetch_add(1, Ordering::Relaxed);
    }

    fn add_to_neighbours(&self, rule: &Rule, index: usize) {
        let (ref value, ref _neighbours) = self[index];

        if *value == rule.states {
            let pos = self.to_pos(index);
            for dir in rule.neighbour_method.get_neighbour_iter() {
                let mut neighbour_pos = pos + *dir;
                keep_in_bounds(rule.bounding_size, &mut neighbour_pos);
                self.increment_neighbor(&neighbour_pos);
            }
        }
    }

    fn calculate_neighbors(&self, task_pool: &TaskPool, rule: &Rule) {
        
        self.clear_neighbors(task_pool);

        let timer = Instant::now();

        let indicies = (0..self.len()).collect::<Vec<_>>();

        indicies.par_chunk_map(task_pool, 1024, |indicies| {
            for index in indicies {
                self.add_to_neighbours(rule, *index);
            }
        });

        info!(
            "calculate_neighbors: {:.3} ms",
            timer.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn clear_neighbors(&self, task_pool: &TaskPool) {
        let timer = Instant::now();

        let slice = &self.data[..];

        slice.par_chunk_map(task_pool, 2048, |states| {
            for (_, ref neighbours) in states {
                neighbours.store(0, Ordering::Relaxed);
            }
        });

        info!(
            "clear_neighbors: {:.3} ms",
            timer.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn update(rule: &Rule, value: &mut u8, neighbours: &u8) {
        if *value != 0 {
            // Decrement value if survival rule isn't passed
            if !(*value == rule.states && rule.survival_rule.in_range(*neighbours)) {
                *value -= 1;
            }
        } else {
            // Check for birth
            if rule.birth_rule.in_range(*neighbours) {
                *value = rule.states;
            }
        }
    }

    fn update_values(&mut self, task_pool: &TaskPool, rule: &Rule) {
        
        let timer = Instant::now();
        
        let mut slice = &mut self.data[..];

        slice.par_chunk_map_mut(task_pool, 1024, |states| {
            for (ref mut value, ref mut neighbours) in states {
                Self::update(rule, value, neighbours.get_mut());
            }
        });

        info!(
            "update_values: {:.3} ms",
            timer.elapsed().as_secs_f64() * 1000.0
        );
    }
}

impl Index<usize> for Cells {
    type Output = (u8, AtomicU8);

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl IndexMut<usize> for Cells {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

/// Spawn all the nessasary `Cell` entities and setup `Cells`
fn init_cells(rule: Res<Rule>, mut commands: Commands) {
    commands.insert_resource(Cells::zeros(&rule))
}

fn spawn_noise(
    keyboard_input: Res<Input<KeyCode>>,
    rule: Res<Rule>,
    mut cells: ResMut<Cells>,
    mut cell_event: EventWriter<UpdateEvent>,
) {
    if keyboard_input.just_pressed(KeyCode::P) {

        cells.spawn_noise(&rule);

        cell_event.send(UpdateEvent);
    }
}

fn tick_cells(
    task_pool: Res<ComputeTaskPool>,
    keyboard_input: Res<Input<KeyCode>>,
    rule: Res<Rule>,
    mut cells: ResMut<Cells>,
    mut cell_event: EventWriter<UpdateEvent>,
) {
    if !keyboard_input.pressed(KeyCode::E) {
        return;
    }

    // Calculate neighbor values in parallel
    cells.calculate_neighbors(&task_pool, &rule);

    // Modifiy all cell values in parallel
    cells.update_values(&task_pool, &rule);

    cell_event.send(UpdateEvent);
}

fn prepare_cell_data(
    rule: Res<Rule>,
    cells: Res<Cells>,
    mut query: Query<&mut InstanceMaterialData>,
) {
    let timer = Instant::now();

    // take the first
    let mut instance_data = query.iter_mut().next().unwrap();
    instance_data.0.clear();
    
    for index in 0..cells.len() {
        let (value, ref neighbours) = cells[index];

        if value != 0 {
            let pos = cells.to_pos(index);
            let neighbours = neighbours.load(Ordering::Relaxed);

            let dist_to_center = utils::dist_to_center(pos, &rule);

            instance_data.0.push(InstanceData {
                position: vec3(pos.x as f32, pos.y as f32, pos.z as f32),
                scale: 1.0,
                color: rule
                    .color_method
                    .color(rule.states, value, neighbours, dist_to_center)
                    .as_rgba_f32(),
                //color: Color::rgba(value as f32 / rule.states as f32, 0.0, 0.0, 1.0)
                //    .as_rgba_f32(),
            });
            
        }
    }

    info!("prepare_cell_data: {:.3} ms", timer.elapsed().as_secs_f64() * 1000.0);
}

/// The system stages to do in order
#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemLabel)]
pub enum Stages {
    SpawnNoise,
    TickCells,
}

pub struct CellsMultithreadedV2Plugin;

impl Plugin for CellsMultithreadedV2Plugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .add_startup_system(init_cells)
            .add_system(spawn_noise.label(Stages::SpawnNoise))
            .add_system(
                tick_cells
                    .label(Stages::TickCells)
                    .after(Stages::SpawnNoise),
            )
            .add_system(prepare_cell_data.after(Stages::TickCells));
    }
}
