//! The scene format: what exists in the world and what it looks like, each
//! object described *whole* (physics and looks together), like a Godot
//! `PackedScene`, a Unity scene or prefab, an Unreal level. The engine
//! defines the format; games define the actual scenes.
//!
//! Loading splits each object: its body goes to the simulation (`world()`),
//! its visual to the renderer. Object `i` becomes physics body `i` and
//! render proxy `i`: that shared index is what links them.

use physics::{Body, DVec2, World};

use crate::render::{Color, Visual};

/// One object of a scene: everything about it, in one place.
// TODO(step-11): becomes an ECS "bundle": the group of components an
// entity is spawned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    /// How it moves and what it collides with: for the simulation.
    pub body: Body,
    /// What it looks like: for the renderer.
    pub visual: Visual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// The objects, in order (the order links bodies and render proxies).
    pub objects: Vec<SceneObject>,
    /// Index of the object the camera follows.
    pub focus: usize,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
    /// The world's gravity, in m/s² (usually `physics::STANDARD_GRAVITY`).
    pub gravity: DVec2,
}

impl Scene {
    /// The physics side of the scene: one body per object, in order.
    pub(crate) fn world(&self) -> World {
        let bodies = self.objects.iter().map(|object| object.body).collect();
        World::new(bodies, self.gravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Shape, WHITE};
    use physics::{Circle, Motion, STANDARD_GRAVITY};

    fn ball(x: f64) -> SceneObject {
        SceneObject {
            body: Body {
                motion: Motion {
                    position: DVec2::new(x, 1.0),
                    velocity: DVec2::ZERO,
                },
                collider: Circle { radius: 1.0 },
            },
            visual: Visual {
                shape: Shape::Circle { radius: 1.0 },
                color: WHITE,
            },
        }
    }

    #[test]
    fn the_world_has_one_body_per_object_in_the_same_order() {
        let scene = Scene {
            objects: vec![ball(1.0), ball(2.0)],
            focus: 0,
            background: WHITE,
            gravity: STANDARD_GRAVITY,
        };
        let world = scene.world();
        assert_eq!(world.bodies, [ball(1.0).body, ball(2.0).body]);
        assert_eq!(world.gravity, STANDARD_GRAVITY);
    }
}
