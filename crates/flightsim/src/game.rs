//! The flight simulator as a game the engine can run: which scenario to
//! play, which key does what, and the game's own HUD lines.

use std::fmt::{self, Write};

use engine::Game;
use engine::input::{Command, InputEvent, Key};
use engine::scene::Scene;
use engine::simulation::Simulation;

use crate::scenarios;

pub struct FlightSim;

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

    fn write_hud(&self, sim: &Simulation, focus: usize, out: &mut String) -> fmt::Result {
        let body = &sim.world().bodies[focus];
        let motion = &body.motion;
        write!(
            out,
            "x = {:.1} m\n\
             height = {:.1} m\n\
             vertical speed = {:.1} m/s\n\
             R: throw again | C: camera | scroll: zoom | \
             L: slow frames | K: stalled frames | Esc: quit",
            motion.position.x,
            // Height of the bottom of the ball, so it reads 0 when resting.
            motion.position.y - body.collider.radius,
            motion.velocity.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{Body, Circle, DVec2, Motion, World};

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
    fn hud_describes_the_focused_ball_and_the_keys() {
        let ball = |x: f64, vy: f64| Body {
            motion: Motion {
                position: DVec2::new(x, 2.4),
                velocity: DVec2::new(0.0, vy),
            },
            collider: Circle { radius: 0.4 },
        };
        let sim = Simulation::new(
            World::new(vec![ball(1.0, 0.0), ball(42.0, -3.0)]),
            1.0 / 120.0,
        );
        let mut out = String::new();
        FlightSim.write_hud(&sim, 1, &mut out).unwrap();
        assert!(out.contains("x = 42.0 m"), "{out}");
        assert!(out.contains("height = 2.0 m"), "{out}"); // bottom of the ball
        assert!(out.contains("vertical speed = -3.0 m/s"), "{out}");
        assert!(out.contains("R: throw again"), "{out}");
    }
}
