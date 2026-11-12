//! The engine: runs a game, one frame at a time.
//!
//! A game plugs in through the [`Game`] trait: its scene (content), how it
//! reacts to input (its controls, its camera) and its own HUD lines.
//! Everything else is the engine's: the simulation with its fixed steps,
//! input translation, rendering and diagnostics. Like MonoGame's `Game`
//! class (`Update` + `Draw` called by the framework's loop) or Bevy's
//! `App`: the engine runs the frame, the game supplies what makes it
//! *this* game.
//!
//! The loop itself (window, clock, events) is the platform's job
//! (`engine_kiss3d::run`); each iteration it calls `input` for every event
//! and then `frame`.

use std::fmt::{self, Write};
use std::time::Duration;

use physics::World;

use crate::hud::{FrameStats, Hud};
use crate::input::{Command, InputEvent, Mouse, RawInput};
use crate::render::{RenderBackend, Renderer, View};
use crate::scene::Scene;
use crate::simulation::{Simulation, WorldView};

/// What a game provides to the engine.
// TODO(step-06): a per-physics-step hook for game logic (mission rules, an
// autopilot), like Unity's `FixedUpdate`, if commands aren't enough.
pub trait Game {
    /// Seconds per physics step: the fixed timestep (e.g. 1/120). A game
    /// setting, like Godot's "physics ticks per second".
    fn physics_dt(&self) -> f64;

    /// The scene to load at start; `Command::Restart` goes back to it.
    fn scene(&self) -> Scene;

    /// Called once the scene is loaded, and again after each restart,
    /// before the next view: everything may have jumped, so a game camera
    /// should cut rather than glide. The game outlives its scene (like
    /// Unity's `DontDestroyOnLoad` objects, which hear of a new scene
    /// through `SceneManager.sceneLoaded`). Optional: does nothing unless
    /// a game needs it.
    fn scene_loaded(&mut self) {}

    /// Reacts to one input event. The bindings and the game's own controls
    /// (its camera; later, its aircraft) are the game's, like Godot's
    /// InputMap and scripts or Unreal's input mappings and
    /// `PlayerController`; engine services are asked for with a `Command`.
    fn input(&mut self, event: InputEvent) -> Option<Command>;

    /// Where to look from and at, for this frame, which lasted
    /// `frame_time` real seconds (0 for the very first picture). The game
    /// moves its camera as it likes, as with a camera script in Unity or
    /// Godot; `world.drawn` says where each body is drawn, so a camera can
    /// follow as smoothly as the picture moves.
    fn view(&mut self, world: WorldView<'_>, frame_time: f64) -> View;

    /// The game's own HUD lines, written under the engine's diagnostics.
    /// The game gets a read-only view of the world's *data* (bodies, time),
    /// never the simulation machinery.
    fn write_hud(&self, world: WorldView<'_>, out: &mut String) -> fmt::Result;
}

/// Whether the game loop should keep going.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flow {
    Continue,
    Quit,
}

/// Debug tool: artificially longer frames, to watch the fixed timestep at
/// work (`Slow`: the simulation keeps real-time pace) and its frame-time
/// cap (`Stalled`: frames beyond `MAX_FRAME_TIME`, sim time falls behind).
#[derive(Clone, Copy, Debug, PartialEq)]
enum DebugLag {
    Off,
    Slow,
    Stalled,
}

impl DebugLag {
    fn delay(self) -> Duration {
        match self {
            DebugLag::Off => Duration::ZERO,
            DebugLag::Slow => Duration::from_millis(50),
            DebugLag::Stalled => Duration::from_millis(400),
        }
    }

    fn name(self) -> &'static str {
        match self {
            DebugLag::Off => "off",
            DebugLag::Slow => "slow frames (+50 ms)",
            DebugLag::Stalled => "stalled frames (+400 ms)",
        }
    }

    /// Asking for a lag turns it on, or off if it's already the current one.
    fn toggled(self, lag: DebugLag) -> DebugLag {
        if self == lag { DebugLag::Off } else { lag }
    }
}

pub struct Engine<G: Game> {
    game: G,
    /// The world as the scene starts, kept to start again on restart.
    start: World,
    sim: Simulation,
    /// Turns raw mouse reports into drags.
    mouse: Mouse,
    /// Physical pixels per logical pixel, as of the latest frame: drags
    /// are converted to logical pixels with it.
    screen_scale: f64,
    renderer: Renderer,
    hud: Hud,
    stats: FrameStats,
    lag: DebugLag,
}

impl<G: Game> Engine<G> {
    /// Loads the game's scene and prepares the first frame (the platform
    /// shows one before the first `frame` call).
    pub fn new(backend: &mut impl RenderBackend, mut game: G) -> Self {
        // TODO(step-09): once scenes come from files, report errors to the
        // player instead of panicking.
        let scene = game.scene();
        let start = scene
            .world()
            .unwrap_or_else(|error| panic!("invalid scene: {error}"));
        let sim = Simulation::new(start.clone(), game.physics_dt());
        let renderer = Renderer::new(backend, &scene);
        game.scene_loaded();
        let view = game.view(sim.view(), 0.0);
        let mut engine = Self {
            game,
            start,
            sim,
            mouse: Mouse::default(),
            screen_scale: f64::from(backend.screen().scale),
            renderer,
            hud: Hud::default(),
            stats: FrameStats::default(),
            lag: DebugLag::Off,
        };
        engine.draw(backend, view, 0);
        engine
    }

    /// Reacts to one raw input report from the platform: translated into a
    /// game-level event, handed to the game.
    pub fn input(&mut self, raw: RawInput) -> Flow {
        let command = self
            .mouse
            .translate(raw, self.screen_scale)
            .and_then(|event| self.game.input(event));
        match command {
            Some(command) => self.execute(command),
            None => Flow::Continue,
        }
    }

    /// One frame lasting `frame_time` real seconds: simulate, ask the game
    /// where to look from, prepare the picture (the platform then shows
    /// it).
    pub fn frame(&mut self, backend: &mut impl RenderBackend, frame_time: f64) {
        self.stats.record(frame_time);
        self.screen_scale = f64::from(backend.screen().scale);
        let steps = self.sim.advance(frame_time);
        let view = self.game.view(self.sim.view(), frame_time);
        self.draw(backend, view, steps);
    }

    /// Extra time the platform should wait each frame (debug lag).
    pub fn debug_delay(&self) -> Duration {
        self.lag.delay()
    }

    /// For tests, to check what the simulation did.
    #[cfg(test)]
    pub(crate) fn simulation(&self) -> &Simulation {
        &self.sim
    }

    /// For tests, to check what the game was told.
    #[cfg(test)]
    pub(crate) fn game(&self) -> &G {
        &self.game
    }

    fn execute(&mut self, command: Command) -> Flow {
        match command {
            Command::Quit => return Flow::Quit,
            Command::Restart => {
                self.sim = Simulation::new(self.start.clone(), self.game.physics_dt());
                self.stats.restart_clock();
                self.game.scene_loaded();
            }
            Command::ToggleSlowFrames => self.lag = self.lag.toggled(DebugLag::Slow),
            Command::ToggleStalledFrames => self.lag = self.lag.toggled(DebugLag::Stalled),
        }
        Flow::Continue
    }

    fn draw(&mut self, backend: &mut impl RenderBackend, view: View, steps: u32) {
        self.renderer.draw(backend, self.sim.view(), view);
        // Writing into a `String` can't fail; an error here would be a bug
        // in a game's `write_hud`.
        self.write_hud(steps)
            .expect("HUD text could not be written");
        self.hud.draw(backend);
    }

    /// The engine's diagnostics, then the game's own lines.
    ///
    /// Watch "sim time" vs "real time": they stay equal at any frame rate,
    /// until frames exceed `MAX_FRAME_TIME` and the simulation falls behind.
    fn write_hud(&mut self, steps: u32) -> fmt::Result {
        let out = self.hud.begin();
        match self.stats.fps() {
            // Frame time (ms) is what developers watch: it adds up across a
            // frame's work, while FPS doesn't.
            Some(fps) => writeln!(out, "{fps:.0} FPS ({:.1} ms per frame)", 1000.0 / fps)?,
            None => writeln!(out, "-- FPS")?,
        }
        writeln!(
            out,
            "{steps} physics steps this frame\n\
             drawn {:.0}% of the way to the next step\n\
             sim time  {:5.2} s\n\
             real time {:5.2} s\n\
             debug lag: {}",
            self.sim.clock().alpha() * 100.0,
            self.sim.world().time(),
            self.stats.real_time(),
            self.lag.name(),
        )
        .and_then(|()| self.game.write_hud(self.sim.view(), out))
    }
}

/// A minimal game for tests: a ground, then balls resting on it at given
/// x positions, simple bindings, a camera fixed to one object, one HUD
/// line of its own. It remembers what it was told, for tests to check.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::input::Key;
    use crate::render::{Color, Material, PlaneSize, Shape, Visual, WHITE};
    use crate::scene::SceneObject;
    use physics::{Body, BodyType, Collider, DVec3, Motion, STANDARD_GRAVITY};

    pub(crate) struct TestGame {
        /// Where the balls start (metres north) and how fast they slide
        /// north (m/s).
        pub balls: Vec<(f64, f64)>,
        /// The object the camera looks at: 0 is the ground, 1 the first
        /// ball.
        pub focus: usize,
        /// The latest input event the game got.
        pub last_event: Option<InputEvent>,
        /// The latest frame time the game's view was asked for.
        pub last_frame_time: Option<f64>,
        /// How many times the game heard its scene was (re)loaded, as of
        /// its latest view.
        pub scenes_loaded: u32,
        loads: u32,
    }

    impl TestGame {
        pub(crate) fn new(balls: Vec<(f64, f64)>, focus: usize) -> Self {
            Self {
                balls,
                focus,
                last_event: None,
                last_frame_time: None,
                scenes_loaded: 0,
                loads: 0,
            }
        }
    }

    /// Where the test camera sits, from what it looks at: 10 m south and
    /// 10 m up (NED).
    const CAMERA_OFFSET: DVec3 = DVec3::new(-10.0, 0.0, -10.0);

    impl Game for TestGame {
        /// 1/64 s: exact in binary floating point, so tests compare exactly.
        fn physics_dt(&self) -> f64 {
            1.0 / 64.0
        }

        fn scene(&self) -> Scene {
            let ground = SceneObject {
                body: Body {
                    body_type: BodyType::Static,
                    motion: Motion {
                        position: DVec3::ZERO,
                        velocity: DVec3::ZERO,
                    },
                    collider: Collider::HalfSpace,
                },
                visual: Visual {
                    shape: Shape::Plane(PlaneSize::Endless),
                    material: Material::plain(WHITE),
                },
            };
            let ball = |&(north, speed): &(f64, f64)| SceneObject {
                body: Body {
                    body_type: BodyType::Dynamic,
                    motion: Motion {
                        position: DVec3::new(north, 0.0, -0.5),
                        velocity: DVec3::new(speed, 0.0, 0.0),
                    },
                    collider: Collider::Sphere { radius: 0.5 },
                },
                visual: Visual {
                    shape: Shape::Sphere { radius: 0.5 },
                    material: Material::plain(WHITE),
                },
            };
            Scene {
                objects: std::iter::once(ground)
                    .chain(self.balls.iter().map(ball))
                    .collect(),
                textures: Vec::new(),
                background: Color::rgb(0.0, 0.0, 0.0),
                gravity: STANDARD_GRAVITY,
            }
        }

        fn scene_loaded(&mut self) {
            self.loads += 1;
        }

        fn input(&mut self, event: InputEvent) -> Option<Command> {
            self.last_event = Some(event);
            match event {
                InputEvent::KeyPressed(Key::Q) => Some(Command::Quit),
                InputEvent::KeyPressed(Key::R) => Some(Command::Restart),
                InputEvent::KeyPressed(Key::L) => Some(Command::ToggleSlowFrames),
                InputEvent::KeyPressed(Key::K) => Some(Command::ToggleStalledFrames),
                _ => None,
            }
        }

        fn view(&mut self, world: WorldView<'_>, frame_time: f64) -> View {
            self.last_frame_time = Some(frame_time);
            self.scenes_loaded = self.loads;
            let target = world.drawn[self.focus];
            View {
                eye: target + CAMERA_OFFSET,
                target,
            }
        }

        fn write_hud(&self, world: WorldView<'_>, out: &mut String) -> fmt::Result {
            let north = world.bodies[self.focus].motion.position.x;
            write!(out, "test game: focus {north:.1} m north")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TestGame;
    use super::*;
    use crate::input::{Key, MouseButton};
    use crate::render::Screen;
    use crate::render::fakes::RecordingBackend;
    use glam::Vec3;
    use physics::DVec2;

    /// Exact in binary floating point.
    const FRAME: f64 = 1.0 / 64.0;

    const SCREEN: Screen = Screen {
        width: 1000.0,
        height: 600.0,
        scale: 1.0,
    };

    /// An engine running `game`, with physics steps of `FRAME`.
    fn engine(game: TestGame) -> (Engine<TestGame>, RecordingBackend) {
        let mut backend = RecordingBackend::new(SCREEN);
        let engine = Engine::new(&mut backend, game);
        (engine, backend)
    }

    /// One ball at x = 0 sliding at 64 m/s: exactly 1 m per physics step.
    fn sliding_ball() -> TestGame {
        TestGame::new(vec![(0.0, 64.0)], 1)
    }

    fn press(engine: &mut Engine<TestGame>, key: Key) -> Flow {
        engine.input(RawInput::KeyPressed(key))
    }

    fn hud_text(backend: &RecordingBackend) -> String {
        let lines: Vec<&str> = backend.texts.iter().map(|(t, _, _)| t.as_str()).collect();
        lines.join("\n")
    }

    fn ball_x(engine: &Engine<TestGame>) -> f64 {
        engine.simulation().world().bodies[1].motion.position.x // 0 is the ground
    }

    #[test]
    fn the_first_frame_is_prepared_on_creation() {
        // The platform shows a frame before the first `frame()` call.
        let (engine, backend) = engine(sliding_ball());
        assert_eq!(backend.proxies.len(), 2); // the ground and the ball
        assert!(backend.camera.is_some());
        assert!(!backend.texts.is_empty());
        assert_eq!(engine.game().last_frame_time, Some(0.0));
    }

    #[test]
    fn the_game_hears_of_each_scene_load_before_its_next_view() {
        // At load, then after a restart: in time for the game to cut its
        // camera before the next picture.
        let (mut engine, mut backend) = engine(sliding_ball());
        assert_eq!(engine.game().scenes_loaded, 1);
        press(&mut engine, Key::R);
        engine.frame(&mut backend, FRAME);
        assert_eq!(engine.game().scenes_loaded, 2);
    }

    #[test]
    fn a_frame_runs_the_physics_steps_its_duration_calls_for() {
        let (mut engine, mut backend) = engine(sliding_ball());
        engine.frame(&mut backend, 2.0 * FRAME);
        assert_eq!(ball_x(&engine), 2.0);
        assert_eq!(engine.simulation().world().time(), 2.0 * FRAME);
    }

    #[test]
    fn the_picture_is_drawn_from_the_view_the_game_chooses() {
        let game = TestGame::new(vec![(1.0, 0.0), (50.0, 0.0)], 2);
        let (mut engine, mut backend) = engine(game);
        engine.frame(&mut backend, FRAME);
        assert_eq!(engine.game().last_frame_time, Some(FRAME));
        // The test game looks at ball 2, resting 50 m north, 0.5 m up: in
        // render axes (east, up, south) that's (0, 0.5, -50). Its eye is
        // 10 m south and 10 m up: +10 in render y and z.
        let (eye, target) = backend.camera.expect("a camera is set");
        assert_eq!(target, Vec3::new(0.0, 0.5, -50.0));
        assert_eq!(eye - target, Vec3::new(0.0, 10.0, 10.0));
    }

    #[test]
    fn the_game_sees_where_bodies_are_drawn_between_steps() {
        // 1.5 steps of time: one step ran (the ball moved from 0 to 1 m
        // north), with half a step left over, so it's drawn halfway
        // between those two states: 0.5 m north, render z = -0.5.
        let (mut engine, mut backend) = engine(sliding_ball());
        engine.frame(&mut backend, 1.5 * FRAME);
        let (_, target) = backend.camera.expect("a camera is set");
        assert_eq!(target.z, -0.5);
    }

    #[test]
    #[should_panic(
        expected = "invalid scene: body 0 is dynamic, but its collider can only be static"
    )]
    fn an_impossible_scene_is_refused_at_load() {
        /// A game whose ground falls: its scene is impossible.
        struct FallingGround;
        impl Game for FallingGround {
            fn physics_dt(&self) -> f64 {
                1.0 / 64.0
            }
            fn scene(&self) -> Scene {
                let mut scene = sliding_ball().scene();
                scene.objects[0].body.body_type = physics::BodyType::Dynamic;
                scene
            }
            fn input(&mut self, _event: InputEvent) -> Option<Command> {
                None
            }
            fn view(&mut self, world: WorldView<'_>, _frame_time: f64) -> View {
                sliding_ball().view(world, 0.0)
            }
            fn write_hud(&self, _: WorldView<'_>, _: &mut String) -> fmt::Result {
                Ok(())
            }
        }
        Engine::new(&mut RecordingBackend::new(SCREEN), FallingGround);
    }

    #[test]
    fn hud_shows_engine_diagnostics_then_the_game_lines() {
        let (mut engine, mut backend) = engine(sliding_ball());
        backend.texts.clear();
        engine.frame(&mut backend, 2.0 * FRAME);
        let text = hud_text(&backend);
        assert!(text.starts_with("-- FPS"), "{text}"); // not measured yet
        assert!(text.contains("2 physics steps this frame"), "{text}");
        assert!(text.ends_with("test game: focus 2.0 m north"), "{text}");
    }

    #[test]
    fn fps_appears_once_half_a_second_of_frames_is_measured() {
        let (mut engine, mut backend) = engine(sliding_ball());
        for _ in 0..32 {
            backend.texts.clear();
            engine.frame(&mut backend, FRAME);
        }
        assert_eq!(backend.texts[0].0, "64 FPS (15.6 ms per frame)");
    }

    #[test]
    fn input_reaches_the_game_as_game_level_events() {
        // Raw reports in, a drag out: the game handles it itself.
        let (mut engine, _) = engine(sliding_ball());
        engine.input(RawInput::CursorMoved(DVec2::new(100.0, 100.0)));
        engine.input(RawInput::MouseButton {
            button: MouseButton::Left,
            pressed: true,
        });
        let flow = engine.input(RawInput::CursorMoved(DVec2::new(130.0, 90.0)));
        assert_eq!(flow, Flow::Continue);
        assert_eq!(
            engine.game().last_event,
            Some(InputEvent::MouseDragged {
                button: MouseButton::Left,
                delta: DVec2::new(30.0, -10.0),
            })
        );
    }

    #[test]
    fn unbound_input_does_nothing() {
        let (mut engine, _) = engine(sliding_ball());
        assert_eq!(press(&mut engine, Key::Z), Flow::Continue);
    }

    #[test]
    fn quit_ends_the_game_loop() {
        let (mut engine, _) = engine(sliding_ball());
        assert_eq!(press(&mut engine, Key::Q), Flow::Quit);
    }

    #[test]
    fn restart_reloads_the_scene_and_restarts_both_clocks() {
        let (mut engine, mut backend) = engine(sliding_ball());
        engine.frame(&mut backend, 4.0 * FRAME);
        assert_eq!(press(&mut engine, Key::R), Flow::Continue);
        assert_eq!(ball_x(&engine), 0.0);
        assert_eq!(engine.simulation().world().time(), 0.0);
        backend.texts.clear();
        engine.frame(&mut backend, 0.0);
        assert!(hud_text(&backend).contains("real time  0.00 s"));
    }

    #[test]
    fn hud_shows_the_debug_lag() {
        let (mut engine, mut backend) = engine(sliding_ball());
        for (key, shown) in [
            (Key::L, "debug lag: slow frames (+50 ms)"),
            (Key::K, "debug lag: stalled frames (+400 ms)"),
            (Key::K, "debug lag: off"),
        ] {
            press(&mut engine, key);
            backend.texts.clear();
            engine.frame(&mut backend, FRAME);
            assert!(hud_text(&backend).contains(shown), "{shown}");
        }
    }

    #[test]
    fn debug_lag_makes_frames_slower_until_switched_off() {
        let (mut engine, _) = engine(sliding_ball());
        assert_eq!(engine.debug_delay(), Duration::ZERO);
        press(&mut engine, Key::L);
        assert_eq!(engine.debug_delay(), Duration::from_millis(50));
        press(&mut engine, Key::K); // switches directly to stalled frames
        assert_eq!(engine.debug_delay(), Duration::from_millis(400));
        press(&mut engine, Key::K);
        assert_eq!(engine.debug_delay(), Duration::ZERO);
    }
}
