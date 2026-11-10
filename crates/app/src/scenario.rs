//! The scenario: what exists in the world and what it looks like, defined
//! in one place. Engines call this a scene or a level (Godot's `.tscn`,
//! Unity scenes, Unreal levels): content described as data, then handed to
//! the simulation (bodies, colliders) and to the renderer (visuals). The
//! game loop only loads it; it never needs to know what's inside.

use physics::{Body, Circle, DVec2, World};

use crate::render::{Color, Shape, Visual};

pub struct Scenario {
    /// Initial physics state: bodies and their colliders.
    pub world: World,
    /// What the ball looks like.
    // TODO(step-11): a `Visual` per entity, stored next to its body and
    // collider (an ECS component).
    pub ball_visual: Visual,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
}

const BALL_RADIUS: f64 = 0.4;

/// A ball resting on the ground, thrown up and forward (m/s): it climbs
/// ~20 m and lands ~33 m away.
// TODO(step-09): scenarios become data files, like the aircraft.
pub fn ball_throw() -> Scenario {
    let ball = Body {
        position: DVec2::new(0.0, BALL_RADIUS),
        velocity: DVec2::new(8.0, 20.0),
    };
    let collider = Circle {
        radius: BALL_RADIUS,
    };
    Scenario {
        world: World::new(ball, collider),
        // Same size as the collider: what you see is what collides.
        ball_visual: Visual {
            shape: Shape::Circle {
                radius: BALL_RADIUS as f32,
            },
            color: Color::rgb(1.0, 0.25, 0.0),
        },
        // Linear light: tiny values for a dark night blue.
        background: Color::rgb(0.006, 0.008, 0.015),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ball_starts_resting_on_the_ground() {
        let scenario = ball_throw();
        let ball = &scenario.world.ball;
        assert_eq!(ball.position.y, scenario.world.ball_shape.radius);
        assert!(ball.velocity.y > 0.0, "thrown upwards");
    }

    #[test]
    fn what_you_see_is_what_collides() {
        // The drawn circle has the collider's size, at render precision:
        // 0.4 isn't exact in binary, so f64 0.4 -> f32 -> f64 gives
        // 0.4000000059..., not 0.4. Compare in f32, what the GPU gets.
        let scenario = ball_throw();
        let Shape::Circle { radius } = scenario.ball_visual.shape;
        assert_eq!(radius, scenario.world.ball_shape.radius as f32);
    }
}
