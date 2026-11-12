//! The game's scenarios: what exists in the world and what it looks like,
//! as engine `Scene`s. Positions and velocities are NED (x north, y east,
//! z down): altitude is -z, and "up" velocities are negative.

use engine::{
    Body, BodyType, Collider, Color, DVec3, Material, Motion, PlaneSize, STANDARD_GRAVITY, Scene,
    SceneObject, Shape, TextureId, TiledTexture, Visual,
};

use crate::textures::{self, GridColors};

/// Flat ground at sea level, where the balls start.
const GROUND_ELEVATION: f64 = 0.0;

/// The ground grid: a line every 2 m, a brighter one every 5 lines (10 m).
const GRID_SPACING: f64 = 2.0;
const GRID_CELLS: u32 = 5;

/// The scene's textures, by index (`TextureId`).
const GRID_TEXTURE: TextureId = TextureId(0);

/// A scene to play, and which of its objects the camera follows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scenario {
    pub(crate) scene: Scene,
    /// Index of the followed object in `scene.objects` (and so of its body
    /// in the world).
    pub(crate) focus: usize,
}

/// Three balls resting on flat ground at sea level, thrown at once in different
/// directions: a small one north-west, a high one north (followed by the
/// camera) and a big flat one north-east. They pass through each other:
/// collisions between objects come in step 12.
// TODO(step-09): scenarios become data files, like the aircraft.
pub(crate) fn ball_throws() -> Scenario {
    let scene = Scene {
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
        textures: vec![textures::grid_tile(
            GRID_CELLS,
            // Linear light: a very dark green-grey, lines in greys.
            GridColors {
                ground: Color::rgb(0.01, 0.012, 0.01),
                minor: Color::rgb(0.08, 0.08, 0.08),
                major: Color::rgb(0.5, 0.5, 0.5),
            },
        )],
        // Linear light: tiny values for a dark night blue.
        background: Color::rgb(0.006, 0.008, 0.015),
        gravity: STANDARD_GRAVITY,
    };
    Scenario {
        scene,
        focus: 2, // the orange ball
    }
}

/// The ground: an endless static plane at `GROUND_ELEVATION`, drawn
/// endless too, covered with the grid texture (one tile per major cell).
fn ground() -> SceneObject {
    SceneObject {
        body: Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 0.0, -GROUND_ELEVATION),
                velocity: DVec3::ZERO,
            },
            collider: Collider::HalfSpace,
        },
        visual: Visual {
            shape: Shape::Plane(PlaneSize::Endless),
            material: Material {
                color: Color::rgb(1.0, 1.0, 1.0), // the texture as it is
                texture: Some(TiledTexture {
                    texture: GRID_TEXTURE,
                    tile_size: GRID_SPACING * f64::from(GRID_CELLS),
                }),
            },
        },
    }
}

/// A ball of `radius` (metres) resting on the ground `east` metres east of
/// the origin, thrown with `velocity` (NED, m/s). Its visual has the
/// collider's size: what you see is what collides.
fn ball(east: f64, radius: f64, velocity: DVec3, color: Color) -> SceneObject {
    SceneObject {
        body: Body {
            body_type: BodyType::Dynamic,
            motion: Motion {
                position: DVec3::new(0.0, east, -GROUND_ELEVATION - radius),
                velocity,
            },
            collider: Collider::Sphere { radius },
        },
        visual: Visual {
            shape: Shape::Sphere {
                radius: radius as f32,
            },
            material: Material::plain(color),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scenario's objects with a body of type `body_type`.
    fn objects(body_type: BodyType) -> Vec<SceneObject> {
        ball_throws()
            .scene
            .objects
            .into_iter()
            .filter(|object| object.body.body_type == body_type)
            .collect()
    }

    /// A ball's radius.
    fn radius(ball: &SceneObject) -> f64 {
        match ball.body.collider {
            Collider::Sphere { radius } => radius,
            Collider::HalfSpace => panic!("a ball is a sphere"),
        }
    }

    #[test]
    fn there_are_three_balls_and_one_ground() {
        assert_eq!(objects(BodyType::Dynamic).len(), 3);
        let statics = objects(BodyType::Static);
        assert_eq!(statics.len(), 1);
        assert_eq!(statics[0].body.collider, Collider::HalfSpace);
        assert_eq!(-statics[0].body.motion.position.z, GROUND_ELEVATION);
    }

    #[test]
    fn every_ball_starts_resting_on_the_ground() {
        // z is down: the centre is `radius` above the ground, at
        // z = -elevation.
        for ball in objects(BodyType::Dynamic) {
            assert_eq!(
                ball.body.motion.position.z,
                -GROUND_ELEVATION - radius(&ball)
            );
        }
    }

    #[test]
    fn every_ball_is_thrown_upwards() {
        for ball in objects(BodyType::Dynamic) {
            assert!(ball.body.motion.velocity.z < 0.0); // -z is up
        }
    }

    #[test]
    fn what_you_see_is_what_collides() {
        // Each drawn sphere has its collider's size, at render precision:
        // 0.4 isn't exact in binary, so f64 0.4 -> f32 -> f64 gives
        // 0.4000000059..., not 0.4. Compare in f32, what the GPU gets.
        for ball in objects(BodyType::Dynamic) {
            let radius = radius(&ball) as f32;
            assert_eq!(ball.visual.shape, Shape::Sphere { radius });
        }
        // The ground's collider is endless: so is what's drawn.
        let ground = objects(BodyType::Static)[0];
        assert_eq!(ground.visual.shape, Shape::Plane(PlaneSize::Endless));
    }

    #[test]
    fn the_ground_texture_exists_and_repeats_every_major_cell() {
        let scene = ball_throws().scene;
        let ground = objects(BodyType::Static)[0];
        let tiled = ground.visual.material.texture.expect("a textured ground");
        assert!(tiled.texture.0 < scene.textures.len());
        // 5 cells of 2 m per tile: major lines every 10 m.
        assert_eq!(tiled.tile_size, 10.0);
    }

    #[test]
    fn the_camera_follows_a_ball() {
        let Scenario { scene, focus } = ball_throws();
        assert_eq!(scene.objects[focus].body.body_type, BodyType::Dynamic);
    }
}
