//! The engine: runs a game, one frame at a time.
//!
//! A game plugs in through the [`Game`] trait: its scene (content), its
//! input bindings and its own HUD lines. Everything else is the engine's:
//! the simulation with its fixed steps, the camera, rendering and
//! diagnostics. Like MonoGame's `Game` class (`Update` + `Draw` called by
//! the framework's loop) or Bevy's `App`: the engine runs the frame, the
//! game supplies what makes it *this* game.
//!
//! The loop itself (window, clock, events) is the platform's job
//! (`engine_kiss3d::run`); each iteration it calls `input` for every event
//! and then `frame`.

use std::fmt::{self, Write};
use std::time::Duration;

use crate::camera::Camera;
use crate::hud::{FrameStats, Hud};
use crate::input::{Command, InputEvent};
use crate::render::{RenderBackend, Renderer, viewport};
use crate::scene::Scene;
use crate::simulation::Simulation;

/// What a game provides to the engine.
pub trait Game {
    /// Seconds per physics step: the fixed timestep (e.g. 1/120). A game
    /// setting, like Godot's "physics ticks per second".
    fn physics_dt(&self) -> f64;

    /// The scene to load at start, and again on `Command::Restart`.
    fn scene(&self) -> Scene;

    /// Input bindings: the command an input event triggers, if any. They
    /// belong to the game, like Godot's InputMap or Unreal's input mapping
    /// contexts.
    fn command_for(&self, event: InputEvent) -> Option<Command>;

    /// The game's own HUD lines, written under the engine's diagnostics.
    /// `focus` is the index of the object the camera follows.
    fn write_hud(&self, sim: &Simulation, focus: usize, out: &mut String) -> fmt::Result;
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
    /// The loaded scene, kept to reload it on restart.
    scene: Scene,
    sim: Simulation,
    /// Where to look and how far zoomed: follows the scene's focus.
    // TODO(step-05): 3D chase and orbit cameras.
    camera: Camera,
    renderer: Renderer,
    hud: Hud,
    stats: FrameStats,
    lag: DebugLag,
}

impl<G: Game> Engine<G> {
    /// Loads the game's scene and prepares the first frame (the platform
    /// shows one before the first `frame` call).
    pub fn new(backend: &mut impl RenderBackend, game: G) -> Self {
        let scene = game.scene();
        let sim = Simulation::new(scene.world(), game.physics_dt());
        let start = scene.objects[scene.focus].body.motion.position;
        let mut camera = Camera::new(start);
        camera.snap_to(start, viewport(backend.screen()));
        let renderer = Renderer::new(backend, &scene);
        let mut engine = Self {
            game,
            scene,
            sim,
            camera,
            renderer,
            hud: Hud::default(),
            stats: FrameStats::default(),
            lag: DebugLag::Off,
        };
        engine.draw(backend, 0);
        engine
    }

    /// Reacts to one input event, through the game's bindings.
    pub fn input(&mut self, event: InputEvent) -> Flow {
        match self.game.command_for(event) {
            Some(command) => self.execute(command),
            None => Flow::Continue,
        }
    }

    /// One frame lasting `frame_time` real seconds: simulate, move the
    /// camera, prepare the picture (the platform then shows it).
    pub fn frame(&mut self, backend: &mut impl RenderBackend, frame_time: f64) {
        self.stats.record(frame_time);
        let steps = self.sim.advance(frame_time);
        // The camera glides towards where the focused object is drawn
        // (interpolated), in real frame time.
        let target = self.sim.drawn_position(self.scene.focus);
        self.camera
            .update(target, frame_time, viewport(backend.screen()));
        self.draw(backend, steps);
    }

    /// Extra time the platform should wait each frame (debug lag).
    pub fn debug_delay(&self) -> Duration {
        self.lag.delay()
    }

    pub fn simulation(&self) -> &Simulation {
        &self.sim
    }

    fn execute(&mut self, command: Command) -> Flow {
        match command {
            Command::Quit => return Flow::Quit,
            Command::Restart => {
                self.sim = Simulation::new(self.scene.world(), self.game.physics_dt());
                self.stats.restart_clock();
            }
            Command::ToggleCamera => self.camera.toggle_mode(),
            Command::Zoom(scroll) => self.camera.zoom_by(scroll),
            Command::ToggleSlowFrames => self.lag = self.lag.toggled(DebugLag::Slow),
            Command::ToggleStalledFrames => self.lag = self.lag.toggled(DebugLag::Stalled),
        }
        Flow::Continue
    }

    fn draw(&mut self, backend: &mut impl RenderBackend, steps: u32) {
        self.renderer.draw(backend, &self.sim, &self.camera);
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
             camera: {}\n\
             debug lag: {}",
            self.sim.clock().alpha() * 100.0,
            self.sim.world().time(),
            self.stats.real_time(),
            self.camera.mode.name(),
            self.lag.name(),
        )
        .and_then(|()| self.game.write_hud(&self.sim, self.scene.focus, out))
    }
}

/// A minimal game for tests: balls resting on the ground at given x
/// positions, simple bindings, one HUD line of its own.
#[cfg(test)]
pub mod testing {
    use super::*;
    use crate::input::Key;
    use crate::render::{Color, Shape, Visual, WHITE};
    use crate::scene::SceneObject;
    use physics::{Body, Circle, DVec2, Motion};

    pub struct TestGame {
        /// Where the balls start (x, metres) and how fast they slide (m/s).
        pub balls: Vec<(f64, f64)>,
        pub focus: usize,
    }

    impl Game for TestGame {
        /// 1/64 s: exact in binary floating point, so tests compare exactly.
        fn physics_dt(&self) -> f64 {
            1.0 / 64.0
        }

        fn scene(&self) -> Scene {
            let ball = |&(x, speed): &(f64, f64)| SceneObject {
                body: Body {
                    motion: Motion {
                        position: DVec2::new(x, 0.5),
                        velocity: DVec2::new(speed, 0.0),
                    },
                    collider: Circle { radius: 0.5 },
                },
                visual: Visual {
                    shape: Shape::Circle { radius: 0.5 },
                    color: WHITE,
                },
            };
            Scene {
                objects: self.balls.iter().map(ball).collect(),
                focus: self.focus,
                background: Color::rgb(0.0, 0.0, 0.0),
            }
        }

        fn command_for(&self, event: InputEvent) -> Option<Command> {
            match event {
                InputEvent::KeyPressed(Key::Q) => Some(Command::Quit),
                InputEvent::KeyPressed(Key::R) => Some(Command::Restart),
                InputEvent::KeyPressed(Key::C) => Some(Command::ToggleCamera),
                InputEvent::KeyPressed(Key::L) => Some(Command::ToggleSlowFrames),
                InputEvent::KeyPressed(Key::K) => Some(Command::ToggleStalledFrames),
                InputEvent::Scroll(amount) => Some(Command::Zoom(amount)),
                InputEvent::KeyPressed(_) => None,
            }
        }

        fn write_hud(&self, sim: &Simulation, focus: usize, out: &mut String) -> fmt::Result {
            let x = sim.world().bodies[focus].motion.position.x;
            write!(out, "test game: focus at x = {x:.1} m")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TestGame;
    use super::*;
    use crate::camera::MAX_ZOOM;
    use crate::input::Key;
    use crate::render::Screen;
    use crate::render::fakes::RecordingBackend;

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
        TestGame {
            balls: vec![(0.0, 64.0)],
            focus: 0,
        }
    }

    fn press(engine: &mut Engine<TestGame>, key: Key) -> Flow {
        engine.input(InputEvent::KeyPressed(key))
    }

    fn hud_text(backend: &RecordingBackend) -> String {
        let lines: Vec<&str> = backend.texts.iter().map(|(t, _, _)| t.as_str()).collect();
        lines.join("\n")
    }

    fn ball_x(engine: &Engine<TestGame>) -> f64 {
        engine.simulation().world().bodies[0].motion.position.x
    }

    #[test]
    fn the_first_frame_is_prepared_on_creation() {
        // The platform shows a frame before the first `frame()` call.
        let (_, backend) = engine(sliding_ball());
        assert_eq!(backend.proxies.len(), 1);
        assert!(backend.camera_at.is_some());
        assert!(!backend.texts.is_empty());
    }

    #[test]
    fn a_frame_runs_the_physics_steps_its_duration_calls_for() {
        let (mut engine, mut backend) = engine(sliding_ball());
        engine.frame(&mut backend, 2.0 * FRAME);
        assert_eq!(ball_x(&engine), 2.0);
        assert_eq!(engine.simulation().world().time(), 2.0 * FRAME);
    }

    #[test]
    fn the_camera_follows_the_scene_focus() {
        let game = TestGame {
            balls: vec![(1.0, 0.0), (50.0, 0.0)],
            focus: 1,
        };
        let (mut engine, mut backend) = engine(game);
        engine.frame(&mut backend, FRAME);
        assert_eq!(backend.camera_at.map(|at| at.x), Some(50.0));
    }

    #[test]
    fn hud_shows_engine_diagnostics_then_the_game_lines() {
        let (mut engine, mut backend) = engine(sliding_ball());
        backend.texts.clear();
        engine.frame(&mut backend, 2.0 * FRAME);
        let text = hud_text(&backend);
        assert!(text.starts_with("-- FPS"), "{text}"); // not measured yet
        assert!(text.contains("2 physics steps this frame"), "{text}");
        assert!(text.contains("camera: follow"), "{text}");
        assert!(text.ends_with("test game: focus at x = 2.0 m"), "{text}");
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
    fn camera_mode_can_be_switched() {
        let (mut engine, mut backend) = engine(sliding_ball());
        press(&mut engine, Key::C);
        backend.texts.clear();
        engine.frame(&mut backend, FRAME);
        assert!(hud_text(&backend).contains("camera: fixed"));
    }

    #[test]
    fn scrolling_zooms_the_camera() {
        let (mut engine, mut backend) = engine(sliding_ball());
        engine.input(InputEvent::Scroll(1e6)); // far in: the camera clamps
        engine.frame(&mut backend, FRAME);
        assert_eq!(backend.zoom, MAX_ZOOM);
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
