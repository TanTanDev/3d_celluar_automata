use std::collections::HashMap;

use bevy::{
    math::{ivec3, IVec3},
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
    let color_1 = color_1.as_rgba_f32();
    let color_2 = color_2.as_rgba_f32();
    let dt = dt.max(0.0).min(1.0);
    let inv = 1.0 - dt;
    let lerped = [
        color_1[0] * dt + color_2[0] * inv,
        color_1[1] * dt + color_2[1] * inv,
        color_1[2] * dt + color_2[2] * inv,
        color_1[3] * dt + color_2[3] * inv,
    ];
    Color::rgba(lerped[0], lerped[1], lerped[2], lerped[3])
}
