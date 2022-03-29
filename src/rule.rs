use bevy::prelude::Color;
use std::ops::RangeInclusive;

use crate::{neighbours::NeighbourMethod, utils};

#[derive(Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMethod {
    Single,
    StateLerp,
    DistToCenter,
    Neighbour,
}

impl ColorMethod {
    pub fn color(&self, c1: Color, c2: Color, states: u8, state: u8, neighbours: u8, dist_to_center: f32) -> Color {
        match self {
            ColorMethod::Single => c1,
            ColorMethod::StateLerp => {
                let dt = state as f32 / states as f32;
                utils::lerp_color(c1, c2, dt)
            }
            ColorMethod::DistToCenter => {
                utils::lerp_color(c1, c2, dist_to_center)
            }
            ColorMethod::Neighbour => {
                let dt = neighbours as f32 / 26f32;
                utils::lerp_color(c1, c2, dt)
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Rule {
    pub survival_rule: Value,
    pub birth_rule: Value,
    pub states: u8,
    pub neighbour_method: NeighbourMethod,
}
