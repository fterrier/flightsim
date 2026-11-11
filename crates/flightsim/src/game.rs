//! The flight simulator as a game the engine can run: which scenario to
//! play, which key does what, and the game's own HUD lines.

use std::fmt::{self, Write};

use engine::{Collider, Command, Game, InputEvent, Key, Scene, WorldView};

use crate::scenarios;

pub(crate) struct FlightSim;

impl Game for FlightSim {
    fn physics_dt(&self) -> f64 {
        1.0 / 120.0
    }

    fn scene(&self) -> Scene {
        scenarios::ball_throws()
    }

    // TODO(step-06): flight controls (throttle, pitch).
    fn command_for(&self, event: InputEvent) -> Option<Command> {
        match event {
            InputEvent::KeyPressed(Key::Escape) => Some(Command::Quit),
            InputEvent::KeyPressed(Key::R) => Some(Command::Restart),
            InputEvent::KeyPressed(Key::C) => Some(Command::ToggleCamera),
            InputEvent::KeyPressed(Key::L) => Some(Command::ToggleSlowFrames),
            InputEvent::KeyPressed(Key::K) => Some(Command::ToggleStalledFrames),
            InputEvent::Scroll(amount) => Some(Command::Zoom(amount)),
            InputEvent::KeyPressed(_) => None,
        }
    }

    fn write_hud(&self, world: WorldView<'_>, focus: usize, out: &mut String) -> fmt::Result {
        // The physics is NED (z down); the HUD speaks like a pilot: altitude
        // and climb rate are positive upwards.
        let body = &world.bodies[focus];
        let (position, velocity) = (body.motion.position, body.motion.velocity);
        // Height of the object's bottom, so a resting ball reads 0.
        let bottom = match body.collider {
            Collider::Sphere { radius } => radius,
            Collider::HalfSpace => 0.0, // its surface
        };
        write!(
            out,
            "north = {:.1} m, east = {:.1} m\n\
             altitude = {:.1} m\n\
             climb rate = {:.1} m/s\n\
             R: throw again | C: camera | scroll: zoom | \
             L: slow frames | K: stalled frames | Esc: quit",
            position.x,
            position.y,
            // Height of the bottom of the ball, so it reads 0 when resting.
            -position.z - bottom,
            -velocity.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Body, BodyType, DVec3, Motion};

    fn command(event: InputEvent) -> Option<Command> {
        FlightSim.command_for(event)
    }

    #[test]
    fn keys_are_bound_to_engine_commands() {
        let press = InputEvent::KeyPressed;
        assert_eq!(command(press(Key::Escape)), Some(Command::Quit));
        assert_eq!(command(press(Key::R)), Some(Command::Restart));
        assert_eq!(command(press(Key::C)), Some(Command::ToggleCamera));
        assert_eq!(command(press(Key::L)), Some(Command::ToggleSlowFrames));
        assert_eq!(command(press(Key::K)), Some(Command::ToggleStalledFrames));
        assert_eq!(command(press(Key::Z)), None);
    }

    #[test]
    fn scrolling_zooms() {
        assert_eq!(command(InputEvent::Scroll(30.0)), Some(Command::Zoom(30.0)));
    }

    #[test]
    fn plays_the_ball_throws_scenario_at_120_hz() {
        assert_eq!(FlightSim.scene(), scenarios::ball_throws());
        assert_eq!(FlightSim.physics_dt(), 1.0 / 120.0);
    }

    #[test]
    fn hud_describes_the_focused_ball_in_flight_terms_and_the_keys() {
        // NED: 42 m north, 7 m east, centre 2.4 m up (z = -2.4), falling
        // at 3 m/s (+z is down).
        let ball = |north: f64, east: f64| Body {
            body_type: BodyType::Dynamic,
            motion: Motion {
                position: DVec3::new(north, east, -2.4),
                velocity: DVec3::new(0.0, 0.0, 3.0),
            },
            collider: Collider::Sphere { radius: 0.4 },
        };
        let bodies = [ball(1.0, 0.0), ball(42.0, 7.0)];
        let world = WorldView {
            time: 0.0,
            bodies: &bodies,
        };
        let mut out = String::new();
        FlightSim.write_hud(world, 1, &mut out).unwrap();
        assert!(out.contains("north = 42.0 m, east = 7.0 m"), "{out}");
        assert!(out.contains("altitude = 2.0 m"), "{out}"); // bottom of the ball
        assert!(out.contains("climb rate = -3.0 m/s"), "{out}"); // falling
        assert!(out.contains("R: throw again"), "{out}");
    }

    #[test]
    fn hud_altitude_of_a_ground_is_its_surface() {
        let plateau = Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 0.0, -12.0),
                velocity: DVec3::ZERO,
            },
            collider: Collider::HalfSpace,
        };
        let world = WorldView {
            time: 0.0,
            bodies: &[plateau],
        };
        let mut out = String::new();
        FlightSim.write_hud(world, 0, &mut out).unwrap();
        assert!(out.contains("altitude = 12.0 m"), "{out}");
    }
}
