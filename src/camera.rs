//! Camera and object matrices shared by the raymarch and mesh passes.
//!
//! The raymarcher builds its rays from the same position, target and field of view,
//! so a depth written by the raymarcher and a depth from a rasterised mesh compare
//! correctly in one depth buffer.

use glam::camera::rh;
use glam::{Mat4, Vec3};

use crate::scene::SceneState;

pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 100.0;

/// Size imported meshes are scaled to (distance from centre to furthest vertex).
pub const MESH_FIT_RADIUS: f32 = 1.3;

pub fn view_projection(scene: &SceneState, aspect: f32) -> Mat4 {
    let view = rh::view::look_at_mat4(
        Vec3::from(scene.camera_position),
        Vec3::from(scene.camera_target),
        Vec3::Y,
    );
    // WebGPU's clip space matches DirectX's: depth 0..1, Y up.
    let projection = rh::proj::directx::perspective(scene.fov_degrees.to_radians(), aspect, NEAR, FAR);
    projection * view
}

/// Centres and scales a mesh to `MESH_FIT_RADIUS`, then turns it slowly about Y.
pub fn mesh_model(centre: [f32; 3], radius: f32, seconds: f32) -> Mat4 {
    let scale = if radius > 0.0 { MESH_FIT_RADIUS / radius } else { 1.0 };
    Mat4::from_rotation_y(seconds * 0.3) * Mat4::from_scale(Vec3::splat(scale)) * Mat4::from_translation(-Vec3::from(centre))
}
