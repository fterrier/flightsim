//! The scene format: everything that exists in the world, as a list of
//! objects, each described *whole* (physics and looks together), plus the
//! scene-wide settings (background, gravity). Like a Godot scene, a Unity
//! scene or prefab, an Unreal level. The engine defines the format; games
//! define the actual scenes.
//!
//! As in engines, the ground is not special: it's an object like a ball,
//! with a body that never moves (a *static* body, like Rapier's fixed
//! bodies or Godot's `StaticBody3D`). Only ambient settings that exist
//! nowhere in particular (background, gravity) are scene fields, like
//! Godot's `WorldEnvironment` or Unity's render and physics settings.
//!
//! Loading splits each object: its body goes to the simulation (`world()`),
//! its visual to the renderer. Object `i` becomes body `i` and drawable
//! `i`: one index everywhere.

use physics::{Body, DVec3, World, WorldError};

use crate::render::{Color, Visual};

/// One object of a scene: everything about it, in one place.
// TODO(step-11): becomes an ECS "bundle": the group of components an
// entity is spawned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    /// How it moves and collides (or doesn't: static bodies): for the
    /// simulation.
    pub body: Body,
    /// What it looks like: for the renderer.
    pub visual: Visual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Everything that exists in the world: moving objects and static ones
    /// (the ground) alike.
    pub objects: Vec<SceneObject>,
    /// Index (in `objects`) of the object the camera follows.
    pub focus: usize,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
    /// The world's gravity, in m/s² (usually `physics::STANDARD_GRAVITY`).
    pub gravity: DVec3,
}

impl Scene {
    /// The physics side of the scene: object `i`'s body is the world's
    /// body `i`. Fails if a body makes no sense (see `WorldError`).
    pub(crate) fn world(&self) -> Result<World, WorldError> {
        let bodies = self.objects.iter().map(|object| object.body).collect();
        World::new(bodies, self.gravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{GroundGrid, Shape, WHITE};
    use physics::{BodyType, Collider, Motion, STANDARD_GRAVITY};

    fn object(body_type: BodyType, collider: Collider, shape: Shape) -> SceneObject {
        SceneObject {
            body: Body {
                body_type,
                motion: Motion {
                    position: DVec3::new(1.0, 2.0, -3.0),
                    velocity: DVec3::ZERO,
                },
                collider,
            },
            visual: Visual {
                shape,
                color: WHITE,
            },
        }
    }

    #[test]
    fn each_object_becomes_the_body_with_its_index() {
        let ground = object(
            BodyType::Static,
            Collider::HalfSpace,
            Shape::GroundPlane(GroundGrid::default()),
        );
        let ball = object(
            BodyType::Dynamic,
            Collider::Sphere { radius: 1.0 },
            Shape::Sphere { radius: 1.0 },
        );
        let scene = Scene {
            objects: vec![ground, ball],
            focus: 1,
            background: WHITE,
            gravity: STANDARD_GRAVITY,
        };
        let world = scene.world().expect("a valid scene");
        assert_eq!(world.bodies, [ground.body, ball.body]);
        assert_eq!(world.gravity, STANDARD_GRAVITY);
    }

    #[test]
    fn a_scene_with_an_impossible_body_has_no_world() {
        let falling_ground = object(
            BodyType::Dynamic,
            Collider::HalfSpace,
            Shape::GroundPlane(GroundGrid::default()),
        );
        let scene = Scene {
            objects: vec![falling_ground],
            focus: 0,
            background: WHITE,
            gravity: STANDARD_GRAVITY,
        };
        assert_eq!(scene.world(), Err(WorldError::MustBeStatic { body: 0 }));
    }
}
