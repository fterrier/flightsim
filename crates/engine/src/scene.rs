//! The scene format: what exists in the world and what it looks like, each
//! object described *whole* (physics and looks together), like a Godot
//! `PackedScene`, a Unity scene or prefab, an Unreal level. The engine
//! defines the format; games define the actual scenes.
//!
//! Loading splits each object: its body goes to the simulation (`world()`),
//! its visual to the renderer. Object `i` becomes physics body `i` and
//! render proxy `i`: that shared index is what links them.

use physics::{Body, DVec3, StaticCollider, World};

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

/// How the ground is drawn: a grid of lines, a brighter one every
/// `major_every` lines, over `half_size` metres around what the camera
/// looks at.
// TODO(later): textured terrain, from real elevation data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    /// Metres between lines.
    pub spacing: f64,
    pub major_every: u32,
    /// Metres drawn on each side of the camera's target.
    pub half_size: f64,
    pub minor_color: Color,
    pub major_color: Color,
}

impl Default for GroundGrid {
    /// A neutral grey grid: a line every 2 m, a brighter one every 10 m,
    /// 50 m around the camera.
    fn default() -> Self {
        Self {
            spacing: 2.0,
            major_every: 5,
            half_size: 50.0,
            minor_color: Color::rgb(0.08, 0.08, 0.08),
            major_color: Color::rgb(0.5, 0.5, 0.5),
        }
    }
}

/// An endless flat ground: what the scene stands on. Content like any
/// object: physics gets a static collider (`StaticCollider::Ground`), the
/// renderer draws its grid. A scene without one has no ground at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Metres above the origin.
    pub elevation: f64,
    /// How it looks.
    pub grid: GroundGrid,
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
    pub gravity: DVec3,
    /// What the scene stands on, if anything.
    pub ground: Option<Ground>,
}

impl Scene {
    /// The physics side of the scene: one body per object, in order, and
    /// the ground as a static collider.
    pub(crate) fn world(&self) -> World {
        let bodies = self.objects.iter().map(|object| object.body).collect();
        let statics = self
            .ground
            .iter()
            .map(|ground| StaticCollider::Ground {
                elevation: ground.elevation,
            })
            .collect();
        World::new(bodies, statics, self.gravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Shape, WHITE};
    use physics::{DVec3, Motion, STANDARD_GRAVITY, Sphere};

    fn ball(x: f64) -> SceneObject {
        SceneObject {
            body: Body {
                motion: Motion {
                    position: DVec3::new(x, 0.0, -1.0),
                    velocity: DVec3::ZERO,
                },
                collider: Sphere { radius: 1.0 },
            },
            visual: Visual {
                shape: Shape::Sphere { radius: 1.0 },
                color: WHITE,
            },
        }
    }

    fn scene(ground: Option<Ground>) -> Scene {
        Scene {
            objects: vec![ball(1.0), ball(2.0)],
            focus: 0,
            background: WHITE,
            gravity: STANDARD_GRAVITY,
            ground,
        }
    }

    #[test]
    fn the_world_has_one_body_per_object_in_the_same_order() {
        let world = scene(None).world();
        assert_eq!(world.bodies, [ball(1.0).body, ball(2.0).body]);
        assert_eq!(world.gravity, STANDARD_GRAVITY);
    }

    #[test]
    fn the_ground_becomes_a_static_collider_at_its_elevation() {
        let ground = Ground {
            elevation: 12.0,
            grid: GroundGrid::default(),
        };
        assert_eq!(
            scene(Some(ground)).world().statics,
            [StaticCollider::Ground { elevation: 12.0 }]
        );
        assert!(scene(None).world().statics.is_empty());
    }
}
