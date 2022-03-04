use std::collections::HashMap;

use bevy::math::{ivec3, IVec3};
use rand::Rng;

use crate::{rule::Rule, State};

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

pub fn spawn_noise(states: &mut HashMap<IVec3, State>, rule: &Rule) {
    let mut rand = rand::thread_rng();
    let spawn_size = 6;
    (0..199).for_each(|_i| {
        states.insert(
            ivec3(
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
                rand.gen_range(-spawn_size..=spawn_size),
            ),
            State::new(rule.start_state_value),
        );
    });
}
