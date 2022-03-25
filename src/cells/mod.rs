use bevy::{tasks::TaskPool};
use crate::{rule::Rule, cell_renderer::CellRenderer};


pub trait Sim: Send + Sync {
    fn update(&mut self, rule: &Rule, task_pool: &TaskPool);
    fn render(&self, data: &mut CellRenderer);

    fn reset(&mut self) {
        let bounds = self.bounds();
        self.set_bounds(0);
        self.set_bounds(bounds);
    }

    fn spawn_noise(&mut self, rule: &Rule);

    fn cell_count(&self) -> usize;

    fn bounds(&self) -> i32;
    fn set_bounds(&mut self, new_bounds: i32) -> i32;
}


pub mod sims;
pub use sims::*;

pub mod tantan;
pub mod leddoo;
