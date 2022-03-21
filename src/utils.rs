use bevy::{
    math::{ivec3, IVec3, Vec4},
    prelude::Color,
};
use rand::Rng;

use crate::{rule::Rule};

pub fn is_in_bounds(pos: IVec3, bounds: i32) -> bool {
    pos.x < bounds && pos.y < bounds && pos.z < bounds
}

pub fn wrap(pos: IVec3, bounds: i32) -> IVec3 {
    // `%` is remainder and keeps negative values negative.
    // we know that negative values are never below -bounds, so we can add
    // bounds to get the modulo (wrapped result is in 0..bounds).
    (pos + bounds) % bounds
}

pub fn dist_to_center(cell_pos: IVec3, rule: &Rule) -> f32 {
    let cell_pos = cell_pos - rule.center();
    let max = rule.bounding_size as f32 / 2.0;
    cell_pos.as_vec3().length() / max
}

pub fn make_some_noise<F: FnMut(IVec3)>(center: IVec3, radius: i32, amount: usize, mut f: F) {
    let mut rand = rand::thread_rng();
    (0..amount).for_each(|_| {
        f(center + ivec3(
            rand.gen_range(-radius..=radius),
            rand.gen_range(-radius..=radius),
            rand.gen_range(-radius..=radius),
        ));
    });
}

pub fn make_some_noise_default<F: FnMut(IVec3)>(center: IVec3, f: F) {
    make_some_noise(center, 6, 12*12*12, f)
}

pub fn lerp_color(color_1: Color, color_2: Color, dt: f32) -> Color {
    let color_1: Vec4 = color_1.into();
    let color_2: Vec4 = color_2.into();
    let dt = dt.clamp(0.0, 1.0);
    ((1.0 - dt)*color_1 + dt*color_2).into()
}


pub fn index_to_pos(index: usize, bound: i32) -> IVec3 {
    ivec3(
        index as i32 % bound,
        index as i32 / bound % bound,
        index as i32 / bound / bound)
}

pub fn pos_to_index(pos: IVec3, bound: i32) -> usize {
    let x = pos.x as usize;
    let y = pos.y as usize;
    let z = pos.z as usize;
    let bound = bound as usize;
    x + y*bound + z*bound*bound
}
