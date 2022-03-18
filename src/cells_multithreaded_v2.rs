use std::collections::HashSet;
use std::hash::Hash;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use bevy::tasks::{ComputeTaskPool, TaskPool};
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

/// The state of all cells last frame
#[derive(Default)]
struct PrevCells {
    states: HashSet<IVec3>,
}

struct Neighbors {
    /// Cache of [`Rule::bounding_size`]
    bounding_size: i32,

    /// 3D arrary of all neighbor values
    data: Vec<Vec<Vec<AtomicU8>>>,
}

impl Neighbors {
    fn zeros(rule: &Rule) -> Self {
        let bounds = rule.get_bounding_ranges();

        let vec = bounds
            .0
            .into_iter()
            .map(|_| {
                bounds
                    .1
                    .clone()
                    .into_iter()
                    .map(|_| {
                        bounds
                            .2
                            .clone()
                            .into_iter()
                            .map(|_| AtomicU8::new(0))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        Self {
            bounding_size: rule.bounding_size,
            data: vec,
        }
    }

    fn inc(&self, index: &IVec3) {
        self.get(index).fetch_add(1, Ordering::Relaxed);
    }

    fn get(&self, index: &IVec3) -> &AtomicU8 {
        let (i, j, k) = self.convert_index(index);
        &self.data[i][j][k]
    }

    fn convert_index(&self, index: &IVec3) -> (usize, usize, usize) {
        (
            (index.x + self.bounding_size).try_into().unwrap(),
            (index.y + self.bounding_size).try_into().unwrap(),
            (index.z + self.bounding_size).try_into().unwrap(),
        )
    }

    fn clear(&self, _task_pool: &TaskPool) {
        // TODO: parallelize
        for val in self.iter() {
            val.store(0, Ordering::Relaxed);
        }
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a AtomicU8> {
        self.data
            .iter()
            .map(|data| data.iter().map(|data| data.iter()))
            .flatten()
            .flatten()
    }
}

/// A single cell
#[derive(Debug, Component)]
struct Cell {
    /// The position index of the cell
    pos: IVec3,

    /// The cell that might be at this position
    ///
    /// Stores (value, neighbours)
    state: Option<(u8, u8)>,
}

impl Cell {
    fn new(pos: IVec3) -> Self {
        Self { pos, state: None }
    }
}

impl From<(i32, i32, i32)> for Cell {
    fn from(pos: (i32, i32, i32)) -> Self {
        Cell::new(pos.into())
    }
}

/// Spawn all the nessasary `Cell` entities and setup `PrevCells`
fn init_cells(rule: Res<Rule>, mut commands: Commands) {
    let bounds = rule.get_bounding_ranges();
    for i in bounds.0 {
        for j in bounds.1.clone() {
            for k in bounds.2.clone() {
                commands.spawn().insert(Cell::from((i, j, k)));
            }
        }
    }
}

/// Spawn hunk of noise if the input was given
fn spawn_noise(keyboard_input: Res<Input<KeyCode>>, mut cells: ResMut<PrevCells>) {
    if keyboard_input.just_pressed(KeyCode::P) {
        info!("Spawn noise");

        // Different version of `utils::spawn_noise`
        let mut rand = rand::thread_rng();
        let spawn_size = 6;
        (0..12 * 12 * 12).for_each(|_i| {
            let pos = ivec3(
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
            );
            cells.states.insert(pos);
        });
    }
}

/// Step all `Cell` entities forward based on `PrevCells` if the input was given
fn tick_cells(
    task_pool: Res<ComputeTaskPool>,
    rule: Res<Rule>,
    keyboard_input: Res<Input<KeyCode>>,
    prev_cells: Res<PrevCells>,
    mut cell: Query<&mut Cell>,
    mut cell_event: EventWriter<UpdateEvent>,
) {
    if !keyboard_input.pressed(KeyCode::E) {
        return;
    }

    let timer = Instant::now();

    let num_threads = 8;
    info!("num_threads: {}", num_threads);
    info!("prev_cells.states.len(): {}", prev_cells.states.len());

    let neighbours = Neighbors::zeros(&rule);

    task_pool.scope(|s| {
        let neighbours = &neighbours;
        let prev_cells = &prev_cells;
        let rule = &rule;

        for thread_id in 0..num_threads {
            s.spawn(async move {
                let mut iter = prev_cells.states.iter();

                // Advance to the threads starting point
                for _ in 0..thread_id {
                    if iter.next().is_none() {
                        // We know the iterator is fused
                        break;
                    }
                }

                for pos in iter.step_by(num_threads) {
                    for dir in rule.neighbour_method.get_neighbour_iter() {
                        let mut neighbour_pos = *pos + *dir;
                        keep_in_bounds(rule.bounding_size, &mut neighbour_pos);
                        neighbours.inc(&neighbour_pos);
                    }
                }
            });
        }
    });

    info!(
        "Tick neighbours: {:.3} ms",
        timer.elapsed().as_secs_f64() * 1000.0
    );

    // Modifiy all cells in parallel in batches of 32
    let timer = Instant::now();
    cell.par_for_each_mut(&task_pool, 32, |mut cell| {
        let neighbour_count = neighbours.get(&cell.pos).load(Ordering::Relaxed);

        if let Some(ref mut cell_state) = cell.state {
            // Decrement value if survival rule isn't passed
            if !(cell_state.0 == rule.states && rule.survival_rule.in_range(neighbour_count)) {
                cell_state.0 -= 1;
                cell_state.1 = neighbour_count;

                // Destroy if no value left
                if cell_state.0 == 0 {
                    cell.state = None;
                }
            }
        } else {
            // Check for birth
            if rule.birth_rule.in_range(neighbour_count) {
                cell.state = Some((rule.states, neighbour_count));
            }
        }
    });

    cell_event.send(UpdateEvent);

    info!(
        "Tick Modifiy: {:.3} ms",
        timer.elapsed().as_secs_f64() * 1000.0
    );
}

/// Save all important `Cell` entities to `PrevCells` for the next frame
fn save(rule: Res<Rule>, mut cells: ResMut<PrevCells>, query: Query<&Cell>) {
    let timer = Instant::now();

    cells.states.clear();

    // TODO: parallelize
    // TODO: could calculate neighbours for next step at this time, then neighbors could be
    // calculated while rendering
    query.for_each(|cell| {
        if let Some(cell_state) = cell.state {
            if cell_state.0 == rule.states {
                cells.states.insert(cell.pos.clone());
            }
        }
    });

    info!("Save: {:.3} ms", timer.elapsed().as_secs_f64() * 1000.0);
}

/// Convert `Cell` entities to `InstanceMaterialData`
fn prepare_cell_data(
    rule: Res<Rule>,
    cells: Query<&Cell>,
    mut query: Query<&mut InstanceMaterialData>,
) {
    let timer = Instant::now();

    // take the first
    let mut instance_data = query.iter_mut().next().unwrap();
    instance_data.0.clear();
    for cell in cells.iter() {
        let pos = cell.pos;

        if let Some(ref state) = cell.state {
            let dist_to_center = utils::dist_to_center(pos, &rule);

            instance_data.0.push(InstanceData {
                position: vec3(pos.x as f32, pos.y as f32, pos.z as f32),
                scale: 1.0,
                color: rule
                    .color_method
                    .color(rule.states, state.0, state.1, dist_to_center)
                    .as_rgba_f32(),
                //color: Color::rgba(state.value as f32 / rule.states as f32, 0.0, 0.0, 1.0)
                //    .as_rgba_f32(),
            });
        }
    }

    info!("Prepare: {:.3} ms", timer.elapsed().as_secs_f64() * 1000.0);
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
        app.init_resource::<PrevCells>()
            .add_startup_system(init_cells)
            .add_system(spawn_noise.label(Stages::SpawnNoise))
            .add_system(
                tick_cells
                    .label(Stages::TickCells)
                    .after(Stages::SpawnNoise),
            )
            .add_system(save.after(Stages::TickCells))
            .add_system(prepare_cell_data.after(Stages::TickCells));
    }
}
