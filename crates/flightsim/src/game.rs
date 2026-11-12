//! The flight simulator as a game the engine can run: which scenario to
//! play, how the player controls it (keys, the camera), and the game's own
//! HUD lines.

use std::fmt::{self, Write};

use engine::{
    Body, Collider, Command, DVec2, Forces, Game, InputEvent, Key, MouseButton, Scene, View,
    WorldView,
};

use crate::camera::Camera;
use crate::scenarios;

/// How hard the movement keys push the ball, in newtons: 4 m/s² on the
/// 1 kg ball.
const PUSH_FORCE: f64 = 4.0;

/// The game's own state: what it follows, its camera, which keys are held.
/// Not the scene: that's built when the engine loads it, and the engine
/// keeps what it needs (as engines load a level from its asset, the game
/// doesn't hold a copy).
pub(crate) struct FlightSim {
    /// Index of the object (and body) the camera follows and the keys push.
    focus: usize,
    /// Follows the focus; the player turns, zooms and switches it.
    camera: Camera,
    /// Which movement keys are down.
    held: Held,
}

impl FlightSim {
    pub(crate) fn new() -> Self {
        Self {
            focus: scenarios::BALL,
            // Put on the focus when the scene loads (`scene_loaded`).
            camera: Camera::new(),
            held: Held::default(),
        }
    }
}

/// The movement keys held down. Input arrives once per frame, pushing
/// happens once per physics step (0, 1 or 2 per frame), so the game
/// remembers what's held between the two: reading a key *press* in the
/// physics step could miss it or see it twice (Unity's docs warn about
/// `GetKeyDown` in `FixedUpdate`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

impl Held {
    /// Records `key` going down (`down`) or up. Returns whether it's a
    /// movement key at all.
    fn set(&mut self, key: Key, down: bool) -> bool {
        let held = match key {
            Key::W => &mut self.forward,
            Key::S => &mut self.back,
            Key::A => &mut self.left,
            Key::D => &mut self.right,
            _ => return false,
        };
        *held = down;
        true
    }

    /// Where the held keys push, relative to the camera: (forward, right),
    /// a unit vector or zero. Two keys at once (diagonally) push no harder
    /// than one; opposite keys cancel out.
    fn direction(self) -> DVec2 {
        let axis = |plus: bool, minus: bool| f64::from(u8::from(plus)) - f64::from(u8::from(minus));
        DVec2::new(axis(self.forward, self.back), axis(self.right, self.left)).normalize_or_zero()
    }
}

impl Game for FlightSim {
    fn physics_dt(&self) -> f64 {
        1.0 / 120.0
    }

    /// Built on demand and handed over: the engine calls it once, at load.
    // TODO(step-09): loaded from the scenario's file instead.
    fn scene(&self) -> Scene {
        scenarios::rolling_ball()
    }

    /// Everything may have jumped (a restart puts the ball back): the
    /// camera jumps to the focus instead of gliding there.
    fn scene_loaded(&mut self, world: WorldView<'_>) {
        let velocity = world.bodies[self.focus].motion.velocity;
        self.camera.jump_to(world.drawn[self.focus], velocity);
    }

    /// Pushes the ball where the held keys say, relative to the camera, as
    /// third-person games do: W away from the camera, D to its right.
    fn fixed_update(&mut self, _bodies: &[Body], forces: &mut Forces<'_>) {
        let keys = self.held.direction(); // (forward, right)
        let forward = self.camera.ground_forward(); // (north, east)
        let right = DVec2::new(-forward.y, forward.x); // a quarter turn clockwise
        let push = (forward * keys.x + right * keys.y) * PUSH_FORCE;
        forces.add(self.focus, push.extend(0.0)); // horizontal: no vertical push
    }

    // TODO(step-06): flight controls (throttle, pitch).
    fn input(&mut self, event: InputEvent) -> Option<Command> {
        match event {
            InputEvent::KeyPressed(key) if self.held.set(key, true) => None,
            InputEvent::KeyReleased(key) => {
                self.held.set(key, false);
                None
            }
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
        let focus = self.focus;
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
        let body = &world.bodies[self.focus];
        let (position, velocity) = (body.motion.position, body.motion.velocity);
        write!(
            out,
            "north = {:.1} m, east = {:.1} m\n\
             ground speed = {:.1} m/s\n\
             altitude = {:.1} m\n\
             climb rate = {:.1} m/s\n\
             camera: {}\n\
             WASD: push | R: restart | C: camera | drag: turn camera | scroll: zoom | \
             L: slow frames | K: stalled frames | Esc: quit",
            position.x,
            position.y,
            velocity.truncate().length(),
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
    use engine::{BodyType, DVec3, Motion};

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

    /// The game once the engine has loaded its scene and told it so.
    fn loaded() -> FlightSim {
        let mut game = FlightSim::new();
        game.scene_loaded(StartWorld::new(&game).view());
        game
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

    /// The force the game puts on the ball in its next physics step.
    fn push(game: &mut FlightSim) -> DVec3 {
        let world = StartWorld::new(game);
        let mut buffer = vec![DVec3::ZERO; world.bodies.len()];
        game.fixed_update(&world.bodies, &mut Forces::new(&mut buffer));
        buffer[scenarios::BALL]
    }

    fn press(game: &mut FlightSim, key: Key) -> Option<Command> {
        game.input(InputEvent::KeyPressed(key))
    }

    fn release(game: &mut FlightSim, key: Key) -> Option<Command> {
        game.input(InputEvent::KeyReleased(key))
    }

    /// Unit vectors (north, east, down) on the ground.
    fn ground(north: f64, east: f64) -> DVec3 {
        DVec3::new(north, east, 0.0).normalize()
    }

    fn assert_close(actual: DVec3, expected: DVec3) {
        assert!(
            (actual - expected).length() < 1e-12,
            "{actual} vs {expected}"
        );
    }

    #[test]
    fn no_key_no_push() {
        assert_eq!(push(&mut loaded()), DVec3::ZERO);
    }

    #[test]
    fn w_pushes_the_ball_away_from_the_camera() {
        // The camera starts south-west of the ball: away is north-east.
        let mut game = loaded();
        assert_eq!(press(&mut game, Key::W), None);
        assert_close(push(&mut game), ground(1.0, 1.0) * PUSH_FORCE);
    }

    #[test]
    fn asd_push_towards_the_camera_and_to_its_sides() {
        let mut game = loaded();
        for (key, north, east) in [
            (Key::S, -1.0, -1.0),
            (Key::A, 1.0, -1.0),
            (Key::D, -1.0, 1.0),
        ] {
            press(&mut game, key);
            assert_close(push(&mut game), ground(north, east) * PUSH_FORCE);
            release(&mut game, key);
        }
    }

    #[test]
    fn the_push_lasts_while_the_key_is_held() {
        let mut game = loaded();
        press(&mut game, Key::W);
        for _ in 0..3 {
            assert_ne!(push(&mut game), DVec3::ZERO, "every step");
        }
        assert_eq!(release(&mut game, Key::W), None);
        assert_eq!(push(&mut game), DVec3::ZERO);
    }

    #[test]
    fn two_keys_push_diagonally_no_harder_and_opposite_keys_cancel() {
        let mut game = loaded();
        press(&mut game, Key::W);
        press(&mut game, Key::D);
        assert!((push(&mut game).length() - PUSH_FORCE).abs() < 1e-12);
        release(&mut game, Key::D);
        press(&mut game, Key::S);
        assert_eq!(push(&mut game), DVec3::ZERO);
    }

    #[test]
    fn the_push_turns_with_the_camera() {
        // Dragged half a turn round: W now pushes south-west.
        let mut game = loaded();
        game.input(InputEvent::MouseDragged {
            button: MouseButton::Left,
            delta: DVec2::new(std::f64::consts::PI / 0.005, 0.0),
        });
        press(&mut game, Key::W);
        assert!((push(&mut game) - ground(-1.0, -1.0) * PUSH_FORCE).length() < 1e-9);
    }

    #[test]
    fn plays_the_rolling_ball_scenario_at_120_hz() {
        let game = FlightSim::new();
        assert_eq!(game.scene(), scenarios::rolling_ball());
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
        assert_eq!(game.input(InputEvent::KeyReleased(Key::Z)), None);
    }

    #[test]
    fn c_switches_the_camera_mode_shown_on_the_hud() {
        let mut game = FlightSim::new();
        assert!(hud(&game).contains("camera: orbit"));
        assert_eq!(game.input(InputEvent::KeyPressed(Key::C)), None);
        assert!(hud(&game).contains("camera: chase"));
    }

    #[test]
    fn once_the_scene_is_loaded_the_camera_is_on_the_focused_ball() {
        let mut game = loaded();
        let ball = scenarios::rolling_ball().objects[scenarios::BALL].body;
        assert_eq!(look(&mut game, 0.0).target, ball.motion.position);
    }

    #[test]
    fn after_a_scene_load_the_camera_jumps_to_the_focus_in_the_same_mode() {
        let mut game = loaded();
        let mut world = StartWorld::new(&game);
        game.input(InputEvent::KeyPressed(Key::C)); // the player's choice

        // The ball is suddenly 1 km away (as after a restart, back at its
        // start): no glide across the map, a jump.
        world.drawn[scenarios::BALL] += DVec3::new(1000.0, 0.0, 0.0);
        game.scene_loaded(world.view());
        let view = game.view(world.view(), 1.0 / 60.0);
        assert_eq!(view.target, world.drawn[scenarios::BALL]);
        assert!(hud(&game).contains("camera: chase"));
    }

    #[test]
    fn dragging_with_the_left_button_turns_the_camera() {
        let mut game = loaded();
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
        let mut game = loaded();
        let before = distance(look(&mut game, 0.0));
        assert_eq!(game.input(InputEvent::Scroll(300.0)), None); // 2x closer
        assert!((distance(look(&mut game, 0.0)) - before / 2.0).abs() < 1e-9);
    }

    #[test]
    fn hud_describes_the_ball_in_flight_terms_and_the_keys() {
        // At the start: at rest on the ground at the origin.
        let out = hud(&FlightSim::new());
        assert!(out.contains("north = 0.0 m, east = 0.0 m"), "{out}");
        assert!(out.contains("ground speed = 0.0 m/s"), "{out}");
        assert!(out.contains("altitude = 0.0 m"), "{out}"); // bottom of the ball
        assert!(out.contains("WASD: push"), "{out}");

        // Rolling 3 m/s north and 4 m/s east (5 m/s over the ground), and
        // rising at 2 m/s (z is down).
        let game = FlightSim::new();
        let mut world = StartWorld::new(&game);
        world.bodies[scenarios::BALL].motion.velocity = DVec3::new(3.0, 4.0, -2.0);
        let mut out = String::new();
        game.write_hud(world.view(), &mut out).unwrap();
        assert!(out.contains("ground speed = 5.0 m/s"), "{out}");
        assert!(out.contains("climb rate = 2.0 m/s"), "{out}");
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
