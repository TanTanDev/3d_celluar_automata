mod cell_renderer;
mod cells_multithreaded;
mod cells_multithreaded_v2;
mod cells_single_threaded;
mod neighbours;
mod rotating_camera;
mod rule;
mod utils;

////////////////////////////////////////////////////////////////////////////////////////////////////

use bevy::{prelude::*, render::view::NoFrustumCulling};

use cell_renderer::*;
// use cells_multithreaded::*;
// use cells_multithreaded_v2::*;
use cells_single_threaded::*;
use neighbours::NeighbourMethod;
use rotating_camera::{RotatingCamera, RotatingCameraPlugin};
use rule::*;

#[derive(Debug)]
pub struct CellState {
    value: u8,
    neighbours: u8,
    dist_to_center: f32,
}

impl CellState {
    pub fn new(value: u8, neighbours: u8, dist_to_center: f32) -> Self {
        CellState {
            value,
            neighbours,
            dist_to_center,
        }
    }
}

fn main() {
    let rule = Rule {
        bounding_size: 50,

        // builder
        survival_rule: Value::Singles(vec![2, 6, 9]),
        birth_rule: Value::Singles(vec![4, 6, 8, 9, 10]),
        states: 10,
        color_method: ColorMethod::DistToCenter(Color::RED, Color::YELLOW),
        neighbour_method: NeighbourMethod::Moore,
        // VN pyramid
        // survival_rule: Value::Range(0..=6),
        // birth_rule: Value::Singles(vec![1,3]),
        // states: 2,
        // color_method: ColorMethod::DistToCenter(Color::BLUE, Color::GREEN),
        // neighbour_method: NeighbourMethod::VonNeuman,

        // fancy snancy
        //survival_rule: Value::Singles(vec![0,1,2,3,7,8,9,11,13,18,21,22,24,26]),
        //birth_rule: Value::Singles(vec![4,13,17,20,21,22,23,24,26]),
        //states: 4,
        //color_method: ColorMethod::StateLerp(Color::BLUE, Color::RED),
        //neighbour_method: NeighbourMethod::Moore,

        // pretty crystals
        // survival_rule: Value::Singles(vec![5,6,7,8]),
        // birth_rule: Value::Singles(vec![6,7,9]),
        // states: 10,
        // color_method: ColorMethod::DistToCenter(Color::BLUE, Color::GREEN),
        //neighbour_method: NeighbourMethod::Moore,

        // swapping structures
        //survival_rule: Value::Singles(vec![3,6,9]),
        //birth_rule: Value::Singles(vec![4,8,10]),
        //states: 20,
        //color_method: ColorMethod::StateLerp(Color::GREEN, Color::RED),
        //neighbour_method: NeighbourMethod::Moore,

        // slowly expanding blob
        //survival_rule: Value::Range(9..=26),
        //birth_rule: Value::Singles(vec![5,6,7,12,13,15]),
        //states: 20,
        //color_method: ColorMethod::StateLerp(Color::BLUE, Color::YELLOW),
        //neighbour_method: NeighbourMethod::Moore,

        // 445
        //survival_rule: Value::Single(4),
        //birth_rule: Value::Single(4),
        //states: 5,
        //color_method: ColorMethod::StateLerp(Color::RED, Color::BLACK),
        //neighbour_method: NeighbourMethod::Moore,

        // expand then die
        //survival_rule: Value::Single(4),
        //birth_rule: Value::Single(3),
        //states: 20,
        //color_method: ColorMethod::StateLerp(Color::RED, Color::BLACK),
        //neighbour_method: NeighbourMethod::Moore,

        // no idea what to call this
        //survival_rule: Value::Singles(vec![6,7]),
        //birth_rule: Value::Singles(vec![4,6,9,10,11]),
        //states: 6,
        //color_method: ColorMethod::StateLerp(Color::RED, Color::BLUE),
        //neighbour_method: NeighbourMethod::Moore,

        // LARGE LINES`
        //survival_rule: Value::Singles(vec![5]),
        //birth_rule: Value::Singles(vec![4, 6, 9, 10, 11, 16, 17, 18, 19, 20, 21, 22, 23, 24]),
        //states: 35,
        //color_method: ColorMethod::StateLerp(Color::RED, Color::BLUE),
        //neighbour_method: NeighbourMethod::Moore,
    };
    let mut task_pool_settings = DefaultTaskPoolOptions::default();
    task_pool_settings.async_compute.percent = 1.0f32;
    task_pool_settings.compute.percent = 0.0f32; // i currently only use async_compute
    task_pool_settings.io.percent = 0.0f32; // always use 1
    App::new()
        .insert_resource(task_pool_settings)
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::rgb(0.65f32, 0.9f32, 0.96f32)))
        .add_plugin(RotatingCameraPlugin)
        .add_plugin(CellMaterialPlugin)
        .insert_resource(rule)
        // you can swap out the different implementations
        // .add_plugin(CellsMultithreadedV2Plugin)
        // .add_plugin(CellsMultithreadedPlugin)
        .add_plugin(CellsSinglethreadedPlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.spawn().insert_bundle((
        meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        InstanceMaterialData(
            (1..=10)
                .flat_map(|x| (1..=100).map(move |y| (x as f32 / 10.0, y as f32 / 10.0)))
                .map(|(x, y)| InstanceData {
                    position: Vec3::new(x * 10.0 - 5.0, y * 10.0 - 5.0, 0.0),
                    scale: 1.0,
                    color: Color::hsla(x * 360., y, 0.5, 1.0).as_rgba_f32(),
                })
                .collect(),
        ),
        Visibility::default(),
        ComputedVisibility::default(),
        // NOTE: Frustum culling is done based on the Aabb of the Mesh and the GlobalTransform.
        // As the cube is at the origin, if its Aabb moves outside the view frustum, all the
        // instanced cubes will be culled.
        // The InstanceMaterialData contains the 'GlobalTransform' information for this custom
        // instancing, and that is not taken into account with the built-in frustum culling.
        // We must disable the built-in frustum culling by adding the `NoFrustumCulling` marker
        // component to avoid incorrect culling.
        NoFrustumCulling,
    ));

    // camera
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        })
        .insert(RotatingCamera::default());
}
