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
}

impl Sims {
    pub fn new() -> Sims {
        Sims {
            sims: vec![],
            active_sim: None,
        }
    }

    pub fn add_sim(&mut self, name: String, sim: Box<dyn Sim>) {
        self.sims.push((name, sim));
    }
}


pub fn update(
    mut sims: ResMut<Sims>,
    rule: Res<Rule>,
    input: Res<Input<KeyCode>>,
    mut query: Query<&mut InstanceMaterialData>,
    task_pool: Res<AsyncComputeTaskPool>,
) {
    let mut new_active = None;
    if input.just_pressed(KeyCode::Key1) { new_active = Some(0); }
    if input.just_pressed(KeyCode::Key2) { new_active = Some(1); }

    if let Some(new_active) = new_active {
        sims.active_sim = Some(new_active);
        sims.sims[new_active].1.reset(&rule);

        println!("switching to {}", sims.sims[new_active].0);
    }

    if let Some(active) = sims.active_sim {
        let sim = &mut sims.sims[active].1;

        sim.update(&input, &rule, &task_pool.0);

        let mut instance_data = query.iter_mut().next().unwrap();
        instance_data.0.clear();

        sim.render(&rule, &mut instance_data.0);
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
