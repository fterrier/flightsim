//! The game's scenarios: what exists in the world and what it looks like,
//! as engine `Scene`s. Positions and velocities are NED (x north, y east,
//! z down): altitude is -z, and "up" velocities are negative.

use engine::{
    Body, Color, DVec3, GroundGrid, Motion, STANDARD_GRAVITY, Scene, SceneBody, SceneObject, Shape,
    Sphere, StaticCollider, Visual,
};

/// Flat ground at sea level, where the balls start.
const GROUND_ELEVATION: f64 = 0.0;

/// Three balls resting on flat ground at sea level, thrown at once in different
/// directions: a small one north-west, a high one north (followed by the
/// camera) and a big flat one north-east. They pass through each other:
/// collisions between objects come in step 12.
// TODO(step-09): scenarios become data files, like the aircraft.
pub(crate) fn ball_throws() -> Scene {
    Scene {
        objects: vec![
            ground(),
            ball(
                -3.0,
                0.3,
                DVec3::new(6.0, -2.0, -14.0),
                Color::rgb(0.1, 0.6, 1.0),
            ),
            ball(
                0.0,
                0.4,
                DVec3::new(8.0, 0.0, -20.0),
                Color::rgb(1.0, 0.25, 0.0),
            ),
            ball(
                3.0,
                0.6,
                DVec3::new(6.0, 6.0, -9.0),
                Color::rgb(1.0, 0.8, 0.1),
            ),
        ],
        focus: 2, // the orange ball
        // Linear light: tiny values for a dark night blue.
        background: Color::rgb(0.006, 0.008, 0.015),
        gravity: STANDARD_GRAVITY,
    }
}

/// The ground: a static object, drawn as a grid with a line every 2 m,
/// a brighter one every 10 m.
fn ground() -> SceneObject {
    SceneObject {
        body: SceneBody::Static(StaticCollider::Ground {
            elevation: GROUND_ELEVATION,
        }),
        visual: Visual {
            shape: Shape::Grid(GroundGrid {
                spacing: 2.0,
                major_every: 5,
                half_size: 50.0,
                minor_color: Color::rgb(0.08, 0.08, 0.08),
            }),
            color: Color::rgb(0.5, 0.5, 0.5),
        },
    }
}

/// A ball of `radius` (metres) resting on the ground `east` metres east of
/// the origin, thrown with `velocity` (NED, m/s). Its visual has the
/// collider's size: what you see is what collides.
fn ball(east: f64, radius: f64, velocity: DVec3, color: Color) -> SceneObject {
    SceneObject {
        body: SceneBody::Dynamic(Body {
            motion: Motion {
                position: DVec3::new(0.0, east, -GROUND_ELEVATION - radius),
                velocity,
            },
            collider: Sphere { radius },
        }),
        visual: Visual {
            shape: Shape::Sphere {
                radius: radius as f32,
            },
            color,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scenario's moving objects.
    fn balls() -> Vec<(Body, Visual)> {
        ball_throws()
            .objects
            .into_iter()
            .filter_map(|object| match object.body {
                SceneBody::Dynamic(body) => Some((body, object.visual)),
                SceneBody::Static(_) => None,
            })
            .collect()
    }

    #[test]
    fn there_are_three_balls_and_a_ground() {
        let scene = ball_throws();
        assert_eq!(balls().len(), 3);
        let grounds: Vec<_> = scene
            .objects
            .iter()
            .filter_map(|object| match object.body {
                SceneBody::Static(StaticCollider::Ground { elevation }) => Some(elevation),
                SceneBody::Dynamic(_) => None,
            })
            .collect();
        assert_eq!(grounds, [GROUND_ELEVATION]);
    }

    #[test]
    fn every_ball_starts_resting_on_the_ground() {
        // z is down: the centre is `radius` above the ground, at
        // z = -elevation.
        for (body, _) in balls() {
            assert_eq!(
                body.motion.position.z,
                -GROUND_ELEVATION - body.collider.radius
            );
        }
    }

    #[test]
    fn every_ball_is_thrown_upwards() {
        for (body, _) in balls() {
            assert!(body.motion.velocity.z < 0.0); // -z is up
        }
    }

    #[test]
    fn what_you_see_is_what_collides() {
        // Each drawn sphere has its collider's size, at render precision:
        // 0.4 isn't exact in binary, so f64 0.4 -> f32 -> f64 gives
        // 0.4000000059..., not 0.4. Compare in f32, what the GPU gets.
        for (body, visual) in balls() {
            assert_eq!(
                visual.shape,
                Shape::Sphere {
                    radius: body.collider.radius as f32
                }
            );
        }
    }

    #[test]
    fn the_camera_follows_a_ball() {
        let scene = ball_throws();
        assert!(matches!(
            scene.objects[scene.focus].body,
            SceneBody::Dynamic(_)
        ));
    }
}
