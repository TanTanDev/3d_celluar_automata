use bevy::{
    prelude::{Plugin, Res, ResMut, Query, Input, KeyCode, Color},
    tasks::AsyncComputeTaskPool,
};
use crate::{
    cells::Sim,
    rule::{Rule, ColorMethod},
    cell_renderer::{InstanceMaterialData, InstanceData, CellRenderer},
    utils,
};


pub struct Sims {
    sims: Vec<(String, Box<dyn Sim>)>,
    active_sim: Option<usize>,
    bounds: i32,
    renderer: Option<Box<CellRenderer>>, // rust...
    color_method: ColorMethod,
}

impl Sims {
    pub fn new() -> Sims {
        Sims {
            sims: vec![],
            active_sim: None,
            bounds: 64,
            renderer: Some(Box::new(CellRenderer::new())),
            color_method: ColorMethod::DistToCenter(Color::YELLOW, Color::RED),
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
    let instance_data = &mut query.iter_mut().next().unwrap().0;

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
            this.sims[active].1.reset();
        }

        if new_active < this.sims.len() {
            this.active_sim = Some(new_active);
            println!("switching to {}", this.sims[new_active].0);
        }
    }

    if let Some(active) = this.active_sim {
        let old_bounds = this.bounds;
        let color_method = this.color_method;
        let mut renderer = this.renderer.take().unwrap();

        let sim = &mut this.sims[active].1;

        let bounds = sim.set_bounds(old_bounds);
        renderer.set_bounds(bounds);

        sim.update(&input, &rule, &task_pool.0);
        sim.render(&mut renderer);

        instance_data.truncate(0);
        for index in 0..renderer.cell_count() {
            let value     = renderer.values[index];
            let neighbors = renderer.neighbors[index];

            if value != 0 {
                let pos = utils::index_to_pos(index, bounds);
                instance_data.push(InstanceData {
                    position: (pos - utils::center(bounds)).as_vec3(),
                    scale: 1.0,
                    color: color_method.color(
                        rule.states,
                        value, neighbors,
                        utils::dist_to_center(pos, bounds),
                    ).into(),
                });
            }
        }

        this.bounds   = bounds;
        this.renderer = Some(renderer);
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
