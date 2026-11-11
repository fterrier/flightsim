//! The flight simulator as a game the engine can run: which scenario to
//! play, how the player controls it (keys, the camera), and the game's own
//! HUD lines.

use std::fmt::{self, Write};

use engine::{Body, Collider, Command, Game, InputEvent, Key, MouseButton, Scene, View, WorldView};

use crate::camera::Camera;
use crate::scenarios::{self, Scenario};

pub(crate) struct FlightSim {
    scenario: Scenario,
    /// Follows the scenario's focus; the player turns, zooms and switches
    /// it.
    camera: Camera,
}

impl FlightSim {
    pub(crate) fn new() -> Self {
        let scenario = scenarios::ball_throws();
        let start = scenario.scene.objects[scenario.focus].body.motion.position;
        Self {
            camera: Camera::new(start),
            scenario,
        }
    }
}

impl Game for FlightSim {
    fn physics_dt(&self) -> f64 {
        1.0 / 120.0
    }

    fn scene(&self) -> Scene {
        self.scenario.scene.clone()
    }

    // TODO(step-06): flight controls (throttle, pitch).
    fn input(&mut self, event: InputEvent) -> Option<Command> {
        match event {
            InputEvent::KeyPressed(Key::Escape) => Some(Command::Quit),
            InputEvent::KeyPressed(Key::R) => Some(Command::Restart),
            InputEvent::KeyPressed(Key::L) => Some(Command::ToggleSlowFrames),
            InputEvent::KeyPressed(Key::K) => Some(Command::ToggleStalledFrames),
            // The camera is the game's own: no engine command needed.
            InputEvent::KeyPressed(Key::C) => {
                self.camera.next_mode();
                None
            }
            InputEvent::Scroll(amount) => {
                self.camera.zoom_by(amount);
                None
            }
            // The left button: the one every trackpad has.
            InputEvent::MouseDragged {
                button: MouseButton::Left,
                delta,
            } => {
                self.camera.orbit(delta);
                None
            }
            InputEvent::KeyPressed(_) | InputEvent::MouseDragged { .. } => None,
        }
    }

    fn view(&mut self, world: WorldView<'_>, frame_time: f64) -> View {
        // The camera glides towards where the focused object is drawn
        // (interpolated), in real frame time; a chase camera also swings
        // round according to its velocity.
        let focus = self.scenario.focus;
        let velocity = world.bodies[focus].motion.velocity;
        self.camera.update(world.drawn[focus], velocity, frame_time);
        View {
            eye: self.camera.eye(),
            target: self.camera.target,
        }
    }

    fn write_hud(&self, world: WorldView<'_>, out: &mut String) -> fmt::Result {
        // The physics is NED (z down); the HUD speaks like a pilot: altitude
        // and climb rate are positive upwards.
        let body = &world.bodies[self.scenario.focus];
        let (position, velocity) = (body.motion.position, body.motion.velocity);
        write!(
            out,
            "north = {:.1} m, east = {:.1} m\n\
             altitude = {:.1} m\n\
             climb rate = {:.1} m/s\n\
             camera: {}\n\
             R: throw again | C: camera | drag: turn camera | scroll: zoom | \
             L: slow frames | K: stalled frames | Esc: quit",
            position.x,
            position.y,
            altitude(body),
            -velocity.z,
            self.camera.mode.name(),
        )
    }
}

/// Height of a body's lowest point, in metres: a ball resting on the
/// ground at sea level reads 0.
fn altitude(body: &Body) -> f64 {
    let bottom = match body.collider {
        Collider::Sphere { radius } => radius,
        Collider::HalfSpace => 0.0, // its surface
    };
    -body.motion.position.z - bottom // z is down
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{BodyType, DVec2, DVec3, Motion};

    /// The game's world as its scene loads: bodies at their start, drawn
    /// where they are.
    struct StartWorld {
        bodies: Vec<Body>,
        drawn: Vec<DVec3>,
    }

    impl StartWorld {
        fn new(game: &FlightSim) -> Self {
            let bodies: Vec<Body> = game.scene().objects.iter().map(|o| o.body).collect();
            let drawn = bodies.iter().map(|b| b.motion.position).collect();
            Self { bodies, drawn }
        }

        fn view(&self) -> WorldView<'_> {
            WorldView {
                time: 0.0,
                bodies: &self.bodies,
                drawn: &self.drawn,
            }
        }
    }

    /// The game's view for a frame of `frame_time` seconds.
    fn look(game: &mut FlightSim, frame_time: f64) -> View {
        let world = StartWorld::new(game);
        game.view(world.view(), frame_time)
    }

    fn hud(game: &FlightSim) -> String {
        let mut out = String::new();
        game.write_hud(StartWorld::new(game).view(), &mut out)
            .unwrap();
        out
    }

    fn distance(view: View) -> f64 {
        view.eye.distance(view.target)
    }

    #[test]
    fn plays_the_ball_throws_scenario_at_120_hz() {
        let game = FlightSim::new();
        assert_eq!(game.scene(), scenarios::ball_throws().scene);
        assert_eq!(game.physics_dt(), 1.0 / 120.0);
    }

    #[test]
    fn keys_ask_the_engine_for_its_services() {
        let mut game = FlightSim::new();
        let mut press = |key| game.input(InputEvent::KeyPressed(key));
        assert_eq!(press(Key::Escape), Some(Command::Quit));
        assert_eq!(press(Key::R), Some(Command::Restart));
        assert_eq!(press(Key::L), Some(Command::ToggleSlowFrames));
        assert_eq!(press(Key::K), Some(Command::ToggleStalledFrames));
        assert_eq!(press(Key::Z), None);
    }

    #[test]
    fn c_switches_the_camera_mode_shown_on_the_hud() {
        let mut game = FlightSim::new();
        assert!(hud(&game).contains("camera: orbit"));
        assert_eq!(game.input(InputEvent::KeyPressed(Key::C)), None);
        assert!(hud(&game).contains("camera: chase"));
    }

    #[test]
    fn the_camera_looks_at_the_focused_ball() {
        let mut game = FlightSim::new();
        let orange = scenarios::ball_throws().scene.objects[2].body;
        assert_eq!(look(&mut game, 0.0).target, orange.motion.position);
    }

    #[test]
    fn dragging_with_the_left_button_turns_the_camera() {
        let mut game = FlightSim::new();
        let before = look(&mut game, 0.0);
        let drag = |button| InputEvent::MouseDragged {
            button,
            delta: DVec2::new(100.0, 0.0),
        };

        assert_eq!(game.input(drag(MouseButton::Right)), None);
        assert_eq!(look(&mut game, 0.0), before, "not the bound button");

        assert_eq!(game.input(drag(MouseButton::Left)), None);
        let after = look(&mut game, 0.0);
        assert_ne!(after.eye, before.eye);
        assert!((distance(after) - distance(before)).abs() < 1e-9);
    }

    #[test]
    fn scrolling_zooms_the_camera() {
        let mut game = FlightSim::new();
        let before = distance(look(&mut game, 0.0));
        assert_eq!(game.input(InputEvent::Scroll(300.0)), None); // 2x closer
        assert!((distance(look(&mut game, 0.0)) - before / 2.0).abs() < 1e-9);
    }

    #[test]
    fn hud_describes_the_focused_ball_in_flight_terms_and_the_keys() {
        // The orange ball at the start: resting at sea level, thrown north
        // at 8 m/s and up at 20 m/s.
        let out = hud(&FlightSim::new());
        assert!(out.contains("north = 0.0 m, east = 0.0 m"), "{out}");
        assert!(out.contains("altitude = 0.0 m"), "{out}"); // bottom of the ball
        assert!(out.contains("climb rate = 20.0 m/s"), "{out}");
        assert!(out.contains("R: throw again"), "{out}");
    }

    #[test]
    fn altitude_is_that_of_the_lowest_point() {
        let at = |z, collider| Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 0.0, z),
                velocity: DVec3::ZERO,
            },
            collider,
        };
        // A ball centred 2.4 m up (z is down): its bottom is 2 m up.
        let ball = at(-2.4, Collider::Sphere { radius: 0.4 });
        assert!((altitude(&ball) - 2.0).abs() < 1e-12);
        // A ground's surface is its position.
        assert_eq!(altitude(&at(-12.0, Collider::HalfSpace)), 12.0);
    }
}
