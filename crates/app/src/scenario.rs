//! The scenario: what exists in the world and what it looks like, defined
//! in one place. Engines call this a scene or a level (Godot's `.tscn`,
//! Unity scenes and prefabs, Unreal levels): each object is described
//! *whole*, physics and looks together. Loading splits it: the bodies go to
//! the simulation (`world()`), the visuals to the renderer. The game loop
//! never needs to know what's inside.

use physics::{Body, Circle, DVec2, Motion, World};

use crate::render::{Color, Shape, Visual};

/// One object of the scenario: everything about it, in one place.
// TODO(step-11): becomes an ECS "bundle": the group of components an
// entity is spawned with.
pub struct SceneObject {
    /// How it moves and what it collides with: for the simulation.
    pub body: Body,
    /// What it looks like: for the renderer.
    pub visual: Visual,
}

pub struct Scenario {
    /// The objects, in order: object `i` becomes physics body `i` and
    /// render proxy `i`. That shared index is what links them.
    pub objects: Vec<SceneObject>,
    /// Index of the object the camera and HUD follow.
    pub focus: usize,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
}

impl Scenario {
    /// The physics side of the scenario: one body per object.
    pub fn world(&self) -> World {
        World::new(self.objects.iter().map(|object| object.body).collect())
    }
}

/// Three balls resting on the ground, thrown at once: a small fast one, a
/// high one (followed by the camera) and a big flat one. They pass through
/// each other: collisions between objects come in step 12.
// TODO(step-09): scenarios become data files, like the aircraft.
pub fn ball_throws() -> Scenario {
    Scenario {
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
    fn the_world_has_one_body_per_object_in_the_same_order() {
        // The order is the link between a body and its render proxy.
        let scenario = ball_throws();
        let bodies: Vec<_> = scenario.objects.iter().map(|o| o.body).collect();
        assert_eq!(scenario.world().bodies, bodies);
    }

    #[test]
    fn the_camera_focuses_on_an_existing_object() {
        let scenario = ball_throws();
        assert!(scenario.focus < scenario.objects.len());
    }
}
