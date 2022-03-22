use bevy::{
    prelude::{Plugin, Res, ResMut, Query, Input, KeyCode},
    tasks::AsyncComputeTaskPool,
};
use crate::{
    cells::Sim, rule::Rule,
    cell_renderer::{InstanceMaterialData},
};


pub struct Sims {
    sims: Vec<(String, Box<dyn Sim>)>,
    active_sim: Option<usize>,
    bounds: i32,
}

impl Sims {
    pub fn new() -> Sims {
        Sims {
            sims: vec![],
            active_sim: None,
            bounds: 64,
        }
    }

    pub fn add_sim(&mut self, name: String, sim: Box<dyn Sim>) {
        self.sims.push((name, sim));
    }
}


pub fn update(
    mut this: ResMut<Sims>,
    rule: Res<Rule>,
    input: Res<Input<KeyCode>>,
    mut query: Query<&mut InstanceMaterialData>,
    task_pool: Res<AsyncComputeTaskPool>,
) {
    let mut new_active = None;
    if input.just_pressed(KeyCode::Key1) { new_active = Some(0); }
    if input.just_pressed(KeyCode::Key2) { new_active = Some(1); }
    if input.just_pressed(KeyCode::Key3) { new_active = Some(2); }
    if input.just_pressed(KeyCode::Key4) { new_active = Some(3); }
    if input.just_pressed(KeyCode::Key5) { new_active = Some(4); }
    if input.just_pressed(KeyCode::Key6) { new_active = Some(5); }
    if input.just_pressed(KeyCode::Key7) { new_active = Some(6); }
    if input.just_pressed(KeyCode::Key8) { new_active = Some(7); }
    if input.just_pressed(KeyCode::Key9) { new_active = Some(8); }
    if input.just_pressed(KeyCode::Key0) { new_active = Some(9); }

    if let Some(new_active) = new_active {
        if let Some(active) = this.active_sim {
            this.sims[active].1.reset(&rule);
        }

        if new_active < this.sims.len() {
            this.active_sim = Some(new_active);
            println!("switching to {}", this.sims[new_active].0);
        }
    }

    if let Some(active) = this.active_sim {
        let bounds = this.bounds;

        let sim = &mut this.sims[active].1;
        let new_bounds = sim.set_bounds(bounds);
        sim.update(&input, &rule, &task_pool.0);

        let mut instance_data = query.iter_mut().next().unwrap();
        instance_data.0.clear();

        sim.render(&rule, &mut instance_data.0);

        this.bounds = new_bounds;
    }
}


pub struct SimsPlugin;
impl Plugin for SimsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app
        .insert_resource(Sims::new())
        .add_system(update);
    }
}
