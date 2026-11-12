//! The game's scenarios: what exists in the world and what it looks like,
//! as engine `Scene`s. Positions and velocities are NED (x north, y east,
//! z down): altitude is -z, and "up" velocities are negative.

use engine::{
    Body, BodyType, Collider, Color, DVec3, Material, Motion, PlaneSize, STANDARD_GRAVITY, Scene,
    SceneObject, Shape, TextureId, TiledTexture, Visual,
};

use crate::textures::{self, GridColors};

/// Flat ground at sea level, where the ball starts.
const GROUND_ELEVATION: f64 = 0.0;

/// The ground grid: a line every 2 m, a brighter one every 5 lines (10 m).
const GRID_SPACING: f64 = 2.0;
const GRID_CELLS: u32 = 5;

/// The scene's textures, by index (`TextureId`).
const GRID_TEXTURE: TextureId = TextureId(0);

/// The ball the player pushes, in `rolling_ball`: its index in the scene's
/// objects (and so of its body in the world).
// TODO(step-11): a reference to an entity instead of an index.
pub(crate) const BALL: usize = 1;

/// The ball's size and mass: about a football's size, a bit heavier.
const BALL_RADIUS: f64 = 0.4;
const BALL_MASS: f64 = 1.0;

/// A ball resting on flat ground at sea level, for the player to push.
// TODO(step-09): scenarios become data files, like the aircraft.
pub(crate) fn rolling_ball() -> Scene {
    Scene {
        objects: vec![ground(), ball()],
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

/// The ball, at rest on the ground at the origin. Its visual has the
/// collider's size: what you see is what collides.
fn ball() -> SceneObject {
    SceneObject {
        body: Body {
            body_type: BodyType::Dynamic { mass: BALL_MASS },
            motion: Motion {
                position: DVec3::new(0.0, 0.0, -GROUND_ELEVATION - BALL_RADIUS),
                velocity: DVec3::ZERO,
            },
            collider: Collider::Sphere {
                radius: BALL_RADIUS,
            },
        },
        visual: Visual {
            shape: Shape::Sphere {
                radius: BALL_RADIUS as f32,
            },
            material: Material::plain(Color::rgb(1.0, 0.25, 0.0)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ball_rests_on_the_endless_ground() {
        let scene = rolling_ball();
        let [ground, ball] = scene.objects[..] else {
            panic!("a ground and a ball");
        };
        // z is down: the ball's centre is its radius above the ground.
        assert_eq!(ground.body.motion.position.z, -GROUND_ELEVATION);
        let ball = ball.body;
        assert_eq!(ball.motion.position.z, -GROUND_ELEVATION - BALL_RADIUS);
        assert_eq!(ball.motion.velocity, DVec3::ZERO);
        assert_eq!(scene.objects[BALL].body, ball);
    }

    #[test]
    fn the_player_pushes_a_moving_ball() {
        let ball = rolling_ball().objects[BALL].body;
        assert_eq!(ball.body_type, BodyType::Dynamic { mass: BALL_MASS });
    }

    #[test]
    fn what_you_see_is_what_collides() {
        let scene = rolling_ball();
        // The ball: a sphere of its collider's size, at render precision
        // (0.4 isn't exact in binary: compare in f32, what the GPU gets).
        let ball = scene.objects[BALL];
        let radius = BALL_RADIUS as f32;
        assert_eq!(
            ball.body.collider,
            Collider::Sphere {
                radius: BALL_RADIUS
            }
        );
        assert_eq!(ball.visual.shape, Shape::Sphere { radius });
        // The ground's collider is endless: so is what's drawn.
        let ground = scene.objects[0];
        assert_eq!(ground.body.collider, Collider::HalfSpace);
        assert_eq!(ground.visual.shape, Shape::Plane(PlaneSize::Endless));
    }

    #[test]
    fn the_ground_texture_exists_and_repeats_every_major_cell() {
        let scene = rolling_ball();
        let tiled = scene.objects[0].visual.material.texture.expect("a texture");
        assert!(tiled.texture.0 < scene.textures.len());
        // 5 cells of 2 m per tile: major lines every 10 m.
        assert_eq!(tiled.tile_size, 10.0);
    }
}
