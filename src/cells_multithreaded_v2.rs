use std::collections::{HashSet, HashMap};
use std::hash::Hash;
use std::sync::{Mutex, Arc};
use std::time::Instant;

use bevy::tasks::ComputeTaskPool;
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

impl PrevCells {
    // Returns the number of neighbours at a given position
    fn neighbour_count(&self, rule: &Rule, pos: &IVec3) -> u8 {
        rule.neighbour_method
            .get_neighbour_iter()
            .iter()
            .filter(|&dir| {
                let mut neighbour_pos = *pos + *dir;
                keep_in_bounds(rule.bounding_size, &mut neighbour_pos);

                self.states.contains(&neighbour_pos)
            })
            .count()
            .try_into()
            .unwrap()
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

    let mut neighbours = HashMap::new();

    for pos in prev_cells.states.iter() {

        for dir in rule.neighbour_method.get_neighbour_iter() {
            let mut neighbour_pos = *pos + *dir;
            keep_in_bounds(rule.bounding_size, &mut neighbour_pos);

            if !neighbours.contains_key(&neighbour_pos) {
                neighbours.insert(neighbour_pos, 0);
            }
            let neighbour = neighbours.get_mut(&neighbour_pos).unwrap();
            *neighbour += 1;
        }
    }
    info!("Tick neighbours: {:.3} ms", timer.elapsed().as_secs_f64() * 1000.0);

    // Modifiy all cells in parallel in batches of 32
    let timer = Instant::now();
    cell.par_for_each_mut(&task_pool, 32, |mut cell| {
        let neighbour_count = neighbours.get(&cell.pos).cloned().unwrap_or(0);

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

    info!("Tick Modifiy: {:.3} ms", timer.elapsed().as_secs_f64() * 1000.0);
}

/// Save all important `Cell` entities to `PrevCells` for the next frame
fn save(rule: Res<Rule>, mut cells: ResMut<PrevCells>, query: Query<&Cell>) {
    let timer = Instant::now();

    cells.states.clear();

    // TODO: parallelize
    // TODO: could calculate neighbours for next step at this time
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
