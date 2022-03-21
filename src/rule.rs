use bevy::prelude::Color;
use std::ops::RangeInclusive;

use bevy::math::{ivec3, IVec3};

use crate::{neighbours::NeighbourMethod, utils};

#[derive(Clone, Copy)]
pub struct Value ([bool; 27]);

impl Value {
    pub fn new(indices: &[u8]) -> Self {
        let mut result = Value([false; 27]);
        for index in indices {
            result.0[*index as usize] = true;
        }
        result
    }

    pub fn from_range(indices: RangeInclusive<u8>) -> Self {
        let mut result = Value([false; 27]);
        for index in indices {
            result.0[index as usize] = true;
        }
        result
    }

    #[allow(dead_code)]
    pub fn in_range(&self, value: u8) -> bool {
        self.0[value as usize]
    }

    pub fn in_range_incorrect(&self, value: u8) -> bool {
        *self.0.get(value as usize).unwrap_or(&false)
    }
}


#[allow(dead_code)]
#[derive(Clone)]
pub enum ColorMethod {
    Single(Color),
    StateLerp(Color, Color),
    DistToCenter(Color, Color),
    Neighbour(Color, Color),
}

impl ColorMethod {
    pub fn color(&self, states: u8, state: u8, neighbours: u8, dist_to_center: f32) -> Color {
        match self {
            ColorMethod::Single(c) => *c,
            ColorMethod::StateLerp(c1, c2) => {
                let dt = state as f32 / states as f32;
                utils::lerp_color(*c1, *c2, dt)
            }
            ColorMethod::DistToCenter(center_c, bounds_c) => {
                utils::lerp_color(*center_c, *bounds_c, dist_to_center)
            }
            ColorMethod::Neighbour(c1, c2) => {
                let dt = neighbours as f32 / 26f32;
                utils::lerp_color(*c1, *c2, dt)
            }
        }
    }
}

#[derive(Clone)]
pub struct Rule {
    pub survival_rule: Value,
    pub birth_rule: Value,
    pub states: u8,
    pub neighbour_method: NeighbourMethod,
    pub bounding_size: i32,
    pub color_method: ColorMethod,
}

impl Rule {
    pub fn get_bounding_ranges(&self) -> (
        RangeInclusive<i32>,
        RangeInclusive<i32>,
        RangeInclusive<i32>,
    ) {
        let x_range = 0..=self.bounding_size-1;
        let y_range = 0..=self.bounding_size-1;
        let z_range = 0..=self.bounding_size-1;
        (x_range, y_range, z_range)
    }

    pub fn center(&self) -> IVec3 {
        let center = self.bounding_size/2;
        ivec3(center, center, center)
    }
}
