//! The game's scenarios: what exists in the world and what it looks like,
//! as engine `Scene`s.

use engine::render::{Color, Shape, Visual};
use engine::scene::{Scene, SceneObject};
use physics::{Body, Circle, DVec2, Motion};

/// Three balls resting on the ground, thrown at once: a small fast one, a
/// high one (followed by the camera) and a big flat one. They pass through
/// each other: collisions between objects come in step 12.
// TODO(step-09): scenarios become data files, like the aircraft.
pub fn ball_throws() -> Scene {
    Scene {
        objects: vec![
            ball(-3.0, 0.3, DVec2::new(6.0, 14.0), Color::rgb(0.1, 0.6, 1.0)),
            ball(0.0, 0.4, DVec2::new(8.0, 20.0), Color::rgb(1.0, 0.25, 0.0)),
            ball(3.0, 0.6, DVec2::new(10.0, 9.0), Color::rgb(1.0, 0.8, 0.1)),
        ],
        focus: 1,
        // Linear light: tiny values for a dark night blue.
        background: Color::rgb(0.006, 0.008, 0.015),
    }
}

/// A ball of `radius` (metres) resting on the ground at `x`, thrown with
/// `velocity` (m/s). Its visual has the collider's size: what you see is
/// what collides.
fn ball(x: f64, radius: f64, velocity: DVec2, color: Color) -> SceneObject {
    SceneObject {
        body: Body {
            motion: Motion {
                position: DVec2::new(x, radius),
                velocity,
            },
            collider: Circle { radius },
        },
        visual: Visual {
            shape: Shape::Circle {
                radius: radius as f32,
            },
            color,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_object_starts_resting_on_the_ground() {
        for object in ball_throws().objects {
            let body = object.body;
            assert_eq!(body.motion.position.y, body.collider.radius);
        }
    }

    #[test]
    fn what_you_see_is_what_collides() {
        // Each drawn circle has its collider's size, at render precision:
        // 0.4 isn't exact in binary, so f64 0.4 -> f32 -> f64 gives
        // 0.4000000059..., not 0.4. Compare in f32, what the GPU gets.
        for object in ball_throws().objects {
            let Shape::Circle { radius } = object.visual.shape;
            assert_eq!(radius, object.body.collider.radius as f32);
        }
    }

    #[test]
    fn the_camera_focuses_on_an_existing_object() {
        let scene = ball_throws();
        assert!(scene.focus < scene.objects.len());
    }
}
