use bevy::{
    prelude::{Plugin, Res, ResMut, Query, Color},
    tasks::AsyncComputeTaskPool,
};
use bevy_egui:: {egui, EguiContext};
use crate::{
    cells::Sim,
    rule::{Rule, ColorMethod},
    cell_renderer::{InstanceMaterialData, InstanceData, CellRenderer},
    utils,
};


pub struct Sims {
    sims: Vec<(String, Box<dyn Sim>)>,
    active_sim: usize,
    bounds: i32,
    renderer: Option<Box<CellRenderer>>, // rust...
    color_method: ColorMethod,
}

impl Sims {
    pub fn new() -> Sims {
        Sims {
            sims: vec![],
            active_sim: usize::MAX,
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
    mut query: Query<&mut InstanceMaterialData>,
    task_pool: Res<AsyncComputeTaskPool>,
    mut egui_context: ResMut<EguiContext>
) {
    let mut bounds = this.bounds;
    let mut active_sim = this.active_sim;
    let mut renderer = this.renderer.take().unwrap();

    if active_sim == usize::MAX {
        assert!(this.sims.len() > 0);
        active_sim = 0;
        this.sims[0].1.set_bounds(bounds);
        this.sims[0].1.spawn_noise(&rule);
        renderer.set_bounds(bounds);
    }

    egui::Window::new("Celluar!").show(egui_context.ctx_mut(), |ui| {
        let old_bounds = bounds;
        let old_active = active_sim;

        ui.label("Simulator:");
        egui::ComboBox::from_id_source("simulator")
            .selected_text(&this.sims[active_sim].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in this.sims.iter().enumerate() {
                    ui.selectable_value(&mut active_sim, i, name);
                }
            }
        );

        if active_sim != old_active {
            this.sims[old_active].1.reset();
            this.sims[active_sim].1.set_bounds(bounds);
            this.sims[active_sim].1.spawn_noise(&rule);
        }


        let sim = &mut this.sims[active_sim].1;

        if ui.button("reset").clicked() {
            sim.reset();
        }
        if ui.button("spawn noise").clicked() {
            sim.spawn_noise(&rule);
        }

        ui.add(egui::Slider::new(&mut bounds, 32..=128)
            .text("bounding size"));
        if bounds != old_bounds {
            bounds = sim.set_bounds(bounds);
            sim.spawn_noise(&rule);
            renderer.set_bounds(bounds);
        }
    });

    let sim = &mut this.sims[active_sim].1;
    sim.update(&rule, &task_pool.0);
    sim.render(&mut renderer);

    let instance_data = &mut query.iter_mut().next().unwrap().0;
    instance_data.truncate(0);
    for index in 0..renderer.cell_count() {
        let value     = renderer.values[index];
        let neighbors = renderer.neighbors[index];

        if value != 0 {
            let pos = utils::index_to_pos(index, bounds);
            instance_data.push(InstanceData {
                position: (pos - utils::center(bounds)).as_vec3(),
                scale: 1.0,
                color: this.color_method.color(
                    rule.states,
                    value, neighbors,
                    utils::dist_to_center(pos, bounds),
                ).into(),
            });
        }
    }

    this.bounds     = bounds;
    this.active_sim = active_sim;
    this.renderer   = Some(renderer);
}


pub struct SimsPlugin;
impl Plugin for SimsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app
        .insert_resource(Sims::new())
        .add_system(update);
    }
}
