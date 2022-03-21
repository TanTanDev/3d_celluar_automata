use bevy::{tasks::TaskPool, prelude::{Input, KeyCode}};
use crate::{rule::Rule, cell_renderer::InstanceData};


pub trait Sim: Send + Sync {
    fn update(&mut self,
        input: &Input<KeyCode>,
        rule: &Rule,
        task_pool: &TaskPool);

    fn render(&self,
        rule: &Rule,
        data: &mut Vec<InstanceData>);

    fn reset(&mut self, rule: &Rule);
}


pub mod sims;
pub use sims::*;

pub mod tantan;
