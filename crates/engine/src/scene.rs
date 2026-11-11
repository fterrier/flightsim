//! The scene format: everything that exists in the world, as a list of
//! objects, each described *whole* (physics and looks together), plus the
//! scene-wide settings (background, gravity). Like a Godot scene, a Unity
//! scene or prefab, an Unreal level. The engine defines the format; games
//! define the actual scenes.
//!
//! As in engines, the ground is not special: it's an object like a ball,
//! one that never moves (a *static* body, like Godot's `StaticBody3D` or
//! Avian's `RigidBody::Static`). Only ambient settings that exist nowhere
//! in particular (background, gravity) are scene fields, like Godot's
//! `WorldEnvironment` or Unity's render and physics settings.
//!
//! Loading splits each object: its body goes to the simulation (`world()`),
//! its visual to the renderer.

use physics::{Body, DVec3, StaticCollider, World};

use crate::render::{Color, Visual};

/// How an object takes part in physics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneBody {
    /// Moves: falls, gets thrown (Avian's `RigidBody::Dynamic`, Godot's
    /// `RigidBody3D`).
    Dynamic(Body),
    /// Never moves: the ground, later buildings (Avian's
    /// `RigidBody::Static`, Godot's `StaticBody3D`).
    Static(StaticCollider),
}

/// One object of a scene: everything about it, in one place.
// TODO(step-11): becomes an ECS "bundle": the group of components an
// entity is spawned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    /// How it takes part in physics: for the simulation.
    pub body: SceneBody,
    /// What it looks like: for the renderer.
    pub visual: Visual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Everything that exists in the world: moving objects and static ones
    /// (the ground) alike.
    pub objects: Vec<SceneObject>,
    /// Index (in `objects`) of the object the camera follows. Must be a
    /// dynamic one.
    pub focus: usize,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
    /// The world's gravity, in m/s² (usually `physics::STANDARD_GRAVITY`).
    pub gravity: DVec3,
}

impl Scene {
    /// The physics side of the scene: dynamic objects become the world's
    /// bodies (in order), static ones its static colliders.
    pub(crate) fn world(&self) -> World {
        let mut bodies = Vec::new();
        let mut statics = Vec::new();
        for object in &self.objects {
            match object.body {
                SceneBody::Dynamic(body) => bodies.push(body),
                SceneBody::Static(collider) => statics.push(collider),
            }
        }
        World::new(bodies, statics, self.gravity)
    }

    /// The index in the world's bodies of object `object`, if it's dynamic
    /// (static objects have no body). Bodies are created in object order,
    /// skipping static objects, so it's the number of dynamic objects
    /// before it.
    // TODO(step-11): entity IDs replace this index arithmetic.
    pub(crate) fn body_index(&self, object: usize) -> Option<usize> {
        let is_dynamic = |o: &SceneObject| matches!(o.body, SceneBody::Dynamic(_));
        is_dynamic(&self.objects[object]).then(|| {
            self.objects[..object]
                .iter()
                .filter(|o| is_dynamic(o))
                .count()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{GroundGrid, Shape, WHITE};
    use physics::{Motion, STANDARD_GRAVITY, Sphere};

    fn ball_body(north: f64) -> Body {
        Body {
            motion: Motion {
                position: DVec3::new(north, 0.0, -1.0),
                velocity: DVec3::ZERO,
            },
            collider: Sphere { radius: 1.0 },
        }
    }

    fn ball(north: f64) -> SceneObject {
        SceneObject {
            body: SceneBody::Dynamic(ball_body(north)),
            visual: Visual {
                shape: Shape::Sphere { radius: 1.0 },
                color: WHITE,
            },
        }
    }

    fn ground(elevation: f64) -> SceneObject {
        SceneObject {
            body: SceneBody::Static(StaticCollider::Ground { elevation }),
            visual: Visual {
                shape: Shape::Grid(GroundGrid::default()),
                color: WHITE,
            },
        }
    }

    fn scene(objects: Vec<SceneObject>) -> Scene {
        Scene {
            objects,
            focus: 0,
            background: WHITE,
            gravity: STANDARD_GRAVITY,
        }
    }

    #[test]
    fn dynamic_objects_become_bodies_in_order() {
        let world = scene(vec![ball(1.0), ball(2.0)]).world();
        assert_eq!(world.bodies, [ball_body(1.0), ball_body(2.0)]);
        assert_eq!(world.gravity, STANDARD_GRAVITY);
        assert!(world.statics.is_empty());
    }

    #[test]
    fn static_objects_become_static_colliders() {
        let world = scene(vec![ground(12.0), ball(1.0)]).world();
        assert_eq!(world.statics, [StaticCollider::Ground { elevation: 12.0 }]);
        assert_eq!(world.bodies, [ball_body(1.0)]);
    }

    #[test]
    fn body_index_skips_static_objects() {
        let scene = scene(vec![ball(1.0), ground(0.0), ball(2.0)]);
        assert_eq!(scene.body_index(0), Some(0));
        assert_eq!(scene.body_index(1), None); // the ground has no body
        assert_eq!(scene.body_index(2), Some(1));
    }
}
