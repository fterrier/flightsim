//! The flight simulator as a game the engine can run: which scenario to
//! play, how the player controls it (keys, the camera), and the game's own
//! HUD lines.

use std::fmt::{self, Write};

use engine::{
    Body, Collider, Command, DVec2, EntityId, Forces, Game, InputEvent, Key, MouseButton, Scene,
    View, WorldView,
};

use crate::camera::Camera;
use crate::scenarios;

/// How hard the movement keys push the ball, in newtons: 4 m/s² on the
/// 1 kg ball.
const PUSH_FORCE: f64 = 4.0;

/// The game's own state: the ball it plays with, its camera, which keys
/// are held. Not the scene: the engine keeps that.
pub(crate) struct FlightSim {
    /// The ball the camera follows and the keys push: the ID the scene
    /// handed out when it was spawned.
    ball: EntityId,
    /// Follows the ball; the player turns, zooms and switches it.
    camera: Camera,
    /// Which movement keys are down.
    held: Held,
}

impl FlightSim {
    /// The game, with its scenario spawned into `scene` (see `Engine::new`).
    pub(crate) fn new(scene: &mut Scene) -> Self {
        Self {
            ball: scenarios::rolling_ball(scene),
            // Put on the ball when the scene loads (`scene_loaded`).
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

    /// Everything may have jumped (a restart puts the ball back): the
    /// camera jumps to the ball instead of gliding there.
    fn scene_loaded(&mut self, world: WorldView<'_>) {
        if let (Some(drawn), Some(body)) = (world.drawn(self.ball), world.body(self.ball)) {
            self.camera.jump_to(drawn, body.motion.velocity);
        }
    }

    /// Pushes the ball where the held keys say, relative to the camera, as
    /// third-person games do: W away from the camera, D to its right.
    fn fixed_update(&mut self, _world: WorldView<'_>, forces: &mut Forces<'_>) {
        let keys = self.held.direction(); // (forward, right)
        let forward = self.camera.ground_forward(); // (north, east)
        let right = DVec2::new(-forward.y, forward.x); // a quarter turn clockwise
        let push = (forward * keys.x + right * keys.y) * PUSH_FORCE;
        forces.add(self.ball, push.extend(0.0)); // horizontal: no vertical push
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
            // Restarting is reloading the scene: everything back at the start.
            InputEvent::KeyPressed(Key::R) => Some(Command::ReloadScene),
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
        // The camera glides towards where the ball is drawn (interpolated),
        // in real frame time; a chase camera also swings round according to
        // its velocity.
        if let (Some(drawn), Some(body)) = (world.drawn(self.ball), world.body(self.ball)) {
            self.camera.update(drawn, body.motion.velocity, frame_time);
        }
        View {
            eye: self.camera.eye(),
            target: self.camera.target,
        }
    }

    fn write_hud(&self, world: WorldView<'_>, out: &mut String) -> fmt::Result {
        // The physics is NED (z down); the HUD speaks like a pilot: altitude
        // and climb rate are positive upwards.
        let Some(body) = world.body(self.ball) else {
            return Ok(()); // no ball, nothing to tell
        };
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
    use engine::{BodyType, DVec3, Flow, Headless, Motion};

    /// The game, run by the real engine without a window.
    fn play() -> Headless<FlightSim> {
        Headless::new(FlightSim::new)
    }

    fn press(game: &mut Headless<FlightSim>, key: Key) -> Flow {
        game.input(InputEvent::KeyPressed(key))
    }

    fn release(game: &mut Headless<FlightSim>, key: Key) -> Flow {
        game.input(InputEvent::KeyReleased(key))
    }

    /// Runs the game for `seconds`, at 60 frames per second.
    fn run(game: &mut Headless<FlightSim>, seconds: f64) {
        for _ in 0..(seconds * 60.0).round() as u32 {
            game.frame(1.0 / 60.0);
        }
    }

    /// The ball's physics state.
    fn ball(game: &Headless<FlightSim>) -> Body {
        *game.world().body(game.game().ball).expect("the ball")
    }

    /// The ball's velocity over the ground (north, east), in m/s.
    fn rolling(game: &Headless<FlightSim>) -> DVec2 {
        ball(game).motion.velocity.truncate()
    }

    fn hud(game: &Headless<FlightSim>) -> String {
        let mut out = String::new();
        game.game().write_hud(game.world(), &mut out).unwrap();
        out
    }

    /// A unit vector (north, east).
    fn heading(north: f64, east: f64) -> DVec2 {
        DVec2::new(north, east).normalize()
    }

    fn assert_close(actual: DVec2, expected: DVec2) {
        assert!(
            (actual - expected).length() < 1e-9,
            "{actual} vs {expected}"
        );
    }

    /// The ball's speed after one second of push, in m/s: `PUSH_FORCE`
    /// (4 N) on the 1 kg ball is 4 m/s², so 4 m/s after 1 s. Exact with
    /// semi-implicit Euler under a constant force (see the physics tests).
    const ONE_SECOND_OF_PUSH: f64 = 4.0;

    #[test]
    fn the_ball_rests_on_the_ground_at_first() {
        let mut game = play();
        run(&mut game, 1.0);
        assert_eq!(rolling(&game), DVec2::ZERO);
        assert_eq!(altitude(&ball(&game)), 0.0);
    }

    #[test]
    fn w_pushes_the_ball_away_from_the_camera() {
        // The camera starts south-west of the ball: away is north-east.
        let mut game = play();
        assert_eq!(press(&mut game, Key::W), Flow::Continue);
        run(&mut game, 1.0);
        assert_close(rolling(&game), heading(1.0, 1.0) * ONE_SECOND_OF_PUSH);
    }

    #[test]
    fn asd_push_towards_the_camera_and_to_its_sides() {
        for (key, north, east) in [
            (Key::S, -1.0, -1.0),
            (Key::A, 1.0, -1.0),
            (Key::D, -1.0, 1.0),
        ] {
            let mut game = play();
            press(&mut game, key);
            run(&mut game, 1.0);
            assert_close(rolling(&game), heading(north, east) * ONE_SECOND_OF_PUSH);
        }
    }

    #[test]
    fn the_push_lasts_only_while_the_key_is_held() {
        // Half a second of push, then it rolls on at what it gained (no
        // friction yet).
        let mut game = play();
        press(&mut game, Key::W);
        run(&mut game, 0.5);
        assert_eq!(release(&mut game, Key::W), Flow::Continue);
        run(&mut game, 0.5);
        assert!((rolling(&game).length() - ONE_SECOND_OF_PUSH / 2.0).abs() < 1e-9);
    }

    #[test]
    fn two_keys_push_diagonally_no_harder_and_opposite_keys_cancel() {
        let mut game = play();
        press(&mut game, Key::W);
        press(&mut game, Key::D);
        run(&mut game, 1.0);
        assert!((rolling(&game).length() - ONE_SECOND_OF_PUSH).abs() < 1e-9);

        let mut game = play();
        press(&mut game, Key::W);
        press(&mut game, Key::S);
        run(&mut game, 1.0);
        assert_eq!(rolling(&game), DVec2::ZERO);
    }

    #[test]
    fn the_push_turns_with_the_camera() {
        // Dragged half a turn round: W now pushes south-west.
        let mut game = play();
        game.input(InputEvent::MouseDragged {
            button: MouseButton::Left,
            delta: DVec2::new(std::f64::consts::PI / 0.005, 0.0),
        });
        press(&mut game, Key::W);
        run(&mut game, 1.0);
        assert!((rolling(&game) - heading(-1.0, -1.0) * ONE_SECOND_OF_PUSH).length() < 1e-6);
    }

    #[test]
    fn r_reloads_the_scene_ball_and_camera_back_at_the_start() {
        let mut game = play();
        let start = ball(&game);
        press(&mut game, Key::C); // the player's camera choice stays
        press(&mut game, Key::W);
        run(&mut game, 1.0);
        assert_eq!(press(&mut game, Key::R), Flow::Continue);
        assert_eq!(ball(&game), start, "the same ball, back at the start");
        assert_eq!(game.game().camera.target, start.motion.position);
        assert!(hud(&game).contains("camera: chase"));
    }

    #[test]
    fn keys_ask_the_engine_for_its_services() {
        // Straight to the game: what it asks of the engine.
        let mut game = FlightSim::new(&mut Scene::default());
        let mut press = |key| game.input(InputEvent::KeyPressed(key));
        assert_eq!(press(Key::Escape), Some(Command::Quit));
        assert_eq!(press(Key::R), Some(Command::ReloadScene));
        assert_eq!(press(Key::L), Some(Command::ToggleSlowFrames));
        assert_eq!(press(Key::K), Some(Command::ToggleStalledFrames));
        assert_eq!(press(Key::Z), None);
        assert_eq!(game.input(InputEvent::KeyReleased(Key::Z)), None);
    }

    #[test]
    fn escape_quits() {
        assert_eq!(press(&mut play(), Key::Escape), Flow::Quit);
    }

    #[test]
    fn once_the_scene_is_loaded_the_camera_is_on_the_ball() {
        let game = play();
        assert_eq!(game.game().camera.target, ball(&game).motion.position);
    }

    #[test]
    fn c_switches_the_camera_mode_shown_on_the_hud() {
        let mut game = play();
        assert!(hud(&game).contains("camera: orbit"));
        press(&mut game, Key::C);
        assert!(hud(&game).contains("camera: chase"));
    }

    #[test]
    fn dragging_with_the_left_button_turns_the_camera() {
        let mut game = play();
        let before = game.game().camera.eye();
        let drag = |button| InputEvent::MouseDragged {
            button,
            delta: DVec2::new(100.0, 0.0),
        };
        game.input(drag(MouseButton::Right));
        assert_eq!(game.game().camera.eye(), before, "not the bound button");
        game.input(drag(MouseButton::Left));
        let after = game.game().camera.eye();
        assert_ne!(after, before);
        let target = game.game().camera.target;
        assert!((after.distance(target) - before.distance(target)).abs() < 1e-9);
    }

    #[test]
    fn scrolling_zooms_the_camera() {
        let mut game = play();
        let distance = |game: &Headless<FlightSim>| {
            let camera = &game.game().camera;
            camera.eye().distance(camera.target)
        };
        let before = distance(&game);
        game.input(InputEvent::Scroll(300.0)); // one "doubling": 2x closer
        assert!((distance(&game) - before / 2.0).abs() < 1e-9);
    }

    #[test]
    fn hud_describes_the_ball_in_flight_terms_and_the_keys() {
        let mut game = play();
        let out = hud(&game);
        assert!(out.contains("north = 0.0 m, east = 0.0 m"), "{out}");
        assert!(out.contains("ground speed = 0.0 m/s"), "{out}");
        assert!(out.contains("altitude = 0.0 m"), "{out}"); // bottom of the ball
        assert!(out.contains("WASD: push"), "{out}");
        press(&mut game, Key::W);
        run(&mut game, 1.0);
        let out = hud(&game);
        assert!(out.contains("ground speed = 4.0 m/s"), "{out}");
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
            restitution: 0.0,
        };
        // A ball centred 2.4 m up (z is down): its bottom is 2 m up.
        let ball = at(-2.4, Collider::Sphere { radius: 0.4 });
        assert!((altitude(&ball) - 2.0).abs() < 1e-12);
        // A ground's surface is its position.
        assert_eq!(altitude(&at(-12.0, Collider::HalfSpace)), 12.0);
    }
}
