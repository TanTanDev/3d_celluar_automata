use std::collections::HashMap;

use bevy::{
    math::{ivec3, IVec3, Vec4},
    prelude::Color,
};
use rand::Rng;

use crate::{rule::Rule, CellState};

// warp coordinates outside of bounds
pub fn keep_in_bounds(bounds: i32, pos: &mut IVec3) {
    //pos.x >= -bounds && pos.y >= -bounds && pos.z >= -bounds && pos.x <= bounds && pos.y <= bounds && pos.z <= bounds
    if pos.x <= -bounds {
        pos.x = bounds - 1;
    } else if pos.x >= bounds {
        pos.x = -bounds + 1;
    }
    if pos.y <= -bounds {
        pos.y = bounds - 1;
    } else if pos.y >= bounds {
        pos.y = -bounds + 1;
    }
    if pos.z <= -bounds {
        pos.z = bounds - 1;
    } else if pos.z >= bounds {
        pos.z = -bounds + 1;
    }
}

pub fn dist_to_center(cell_pos: IVec3, rule: &Rule) -> f32 {
    let max = rule.bounding_size as f32;
    cell_pos.as_vec3().length() / max
}

pub fn spawn_noise(states: &mut HashMap<IVec3, CellState>, rule: &Rule) {
    let mut rand = rand::thread_rng();
    let spawn_size = 6;
    (0..12 * 12 * 12).for_each(|_i| {
        let pos = ivec3(
            rand.gen_range(-spawn_size..=spawn_size),
            rand.gen_range(-spawn_size..=spawn_size),
            rand.gen_range(-spawn_size..=spawn_size),
        );
        let dist = dist_to_center(pos, rule);
        states.insert(pos, CellState::new(rule.states, 0, dist));
    });
}

pub fn spawn_noise_small(states: &mut HashMap<IVec3, CellState>, rule: &Rule) {
    let mut rand = rand::thread_rng();
    let spawn_size = 1;
    (0..12 * 12 * 12).for_each(|_i| {
        let pos = ivec3(
            rand.gen_range(-spawn_size..=spawn_size),
            rand.gen_range(-spawn_size..=spawn_size),
            rand.gen_range(-spawn_size..=spawn_size),
        );
        let dist = dist_to_center(pos, rule);
        states.insert(pos, CellState::new(rule.states, 0, dist));
    });
}

pub fn lerp_color(color_1: Color, color_2: Color, dt: f32) -> Color {
    let color_1: Vec4 = color_1.into();
    let color_2: Vec4 = color_2.into();
    let dt = dt.clamp(0.0, 1.0);
    ((1.0 - dt)*color_1 + dt*color_2).into()
}
