use bevy::math::{vec3, Quat};
use bevy::prelude::*;

#[derive(Component)]
pub struct RotatingCamera {
    pub rotation: f32,
    pub last_tick: f32,
    pub speed: f32,
    pub dist: f32,
    pub center: Vec3,
}

impl Default for RotatingCamera {
    fn default() -> Self {
        Self {
            rotation: 0f32,
            last_tick: 0f32,
            speed: 0.01f32,
            dist: 150f32,
            center: vec3(0.0, 0.0, 0.0),
        }
    }
}

pub struct RotatingCameraPlugin;
impl Plugin for RotatingCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(update_tick);
    }
}

pub fn update_tick(
    mut cameras: Query<(&mut RotatingCamera, &mut Transform)>,
) {
    for (mut camera, mut transform) in cameras.iter_mut() {
        let delta = 1.0f32;
        camera.rotation += delta * camera.speed;
        let rotation = Quat::from_axis_angle(Vec3::Y, camera.rotation);
        transform.translation = camera.center + (rotation * Vec3::Z * camera.dist);
        transform.look_at(camera.center, Vec3::Y);
    }
}
