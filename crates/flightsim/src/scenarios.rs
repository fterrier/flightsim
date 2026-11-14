//! The game's scenarios: what exists in the world and what it looks like,
//! spawned into an engine `Scene`. Positions and velocities are NED (x
//! north, y east, z down): altitude is -z, and "up" velocities are
//! negative.

use engine::{
    Body, BodyType, Collider, Color, DVec3, EntityId, Material, Motion, PlaneSize,
    STANDARD_GRAVITY, Scene, SceneObject, Shape, TextureId, TiledTexture, Visual,
};

use crate::textures::{self, GridColors};

/// Flat ground at sea level, where the ball starts.
const GROUND_ELEVATION: f64 = 0.0;

/// The ground grid: a line every 2 m, a brighter one every 5 lines (10 m).
const GRID_SPACING: f64 = 2.0;
const GRID_CELLS: u32 = 5;

/// The ball's size and mass: about a football's size, a bit heavier.
const BALL_RADIUS: f64 = 0.4;
const BALL_MASS: f64 = 1.0;

/// Spawns a ball resting on flat ground at sea level, for the player to
/// push, and returns the ball's ID.
// TODO(step-09): scenarios become data files, like the aircraft.
pub(crate) fn rolling_ball(scene: &mut Scene) -> EntityId {
    // Linear light: tiny values for a dark night blue.
    scene.background = Color::rgb(0.006, 0.008, 0.015);
    scene.gravity = STANDARD_GRAVITY;
    let grid = scene.add_texture(textures::grid_tile(
        GRID_CELLS,
        // Linear light: a very dark green-grey, lines in greys.
        GridColors {
            ground: Color::rgb(0.01, 0.012, 0.01),
            minor: Color::rgb(0.08, 0.08, 0.08),
            major: Color::rgb(0.5, 0.5, 0.5),
        },
    ));
    scene.spawn(ground(grid));
    scene.spawn(ball())
}

/// The ground: an endless static plane at `GROUND_ELEVATION`, drawn
/// endless too, covered with `grid` (one tile per major cell).
fn ground(grid: TextureId) -> SceneObject {
    SceneObject {
        body: Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 0.0, -GROUND_ELEVATION),
                velocity: DVec3::ZERO,
            },
            collider: Collider::HalfSpace,
            restitution: 0.0,
        },
        visual: Visual {
            shape: Shape::Plane(PlaneSize::Endless),
            material: Material {
                color: Color::rgb(1.0, 1.0, 1.0), // the texture as it is
                texture: Some(TiledTexture {
                    texture: grid,
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
            restitution: 0.9,
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

    /// A texture ID for the ground to refer to.
    fn some_texture() -> TextureId {
        let black = Color::rgb(0.0, 0.0, 0.0);
        let colors = GridColors {
            ground: black,
            minor: black,
            major: black,
        };
        Scene::default().add_texture(textures::grid_tile(1, colors))
    }

    #[test]
    fn the_ball_rests_on_the_ground_and_can_be_pushed() {
        // z is down: the ball's centre is its radius above the ground.
        let ball = ball().body;
        let ground = ground(some_texture()).body;
        assert_eq!(
            ball.motion.position.z,
            ground.motion.position.z - BALL_RADIUS
        );
        assert_eq!(ball.motion.velocity, DVec3::ZERO);
        assert_eq!(ball.body_type, BodyType::Dynamic { mass: BALL_MASS });
    }

    #[test]
    fn what_you_see_is_what_collides() {
        // The ball: a sphere of its collider's size, at render precision
        // (0.4 isn't exact in binary: compare in f32, what the GPU gets).
        let ball = ball();
        let radius = BALL_RADIUS as f32;
        let collider = Collider::Sphere {
            radius: BALL_RADIUS,
        };
        assert_eq!(ball.body.collider, collider);
        assert_eq!(ball.visual.shape, Shape::Sphere { radius });
        // The ground's collider is endless: so is what's drawn.
        let ground = ground(some_texture());
        assert_eq!(ground.body.collider, Collider::HalfSpace);
        assert_eq!(ground.visual.shape, Shape::Plane(PlaneSize::Endless));
    }

    #[test]
    fn the_ground_is_covered_with_its_grid_every_major_cell() {
        let grid = some_texture();
        let tiled = ground(grid).visual.material.texture.expect("a texture");
        assert_eq!(tiled.texture, grid);
        // 5 cells of 2 m per tile: major lines every 10 m.
        assert_eq!(tiled.tile_size, 10.0);
    }
}
