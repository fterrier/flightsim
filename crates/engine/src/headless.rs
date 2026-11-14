//! Running a game without a window or a GPU: the whole engine, simulation
//! and game logic included, drawing nothing. Like Godot's `--headless`
//! (a dummy renderer) or Bevy without its render plugins. What a game's
//! own tests run on; later, replays checked in CI, or a server.

use arena::Arena;
use glam::{Vec2, Vec3};

use crate::engine::{Engine, Flow, Game};
use crate::input::InputEvent;
use crate::render::{Color, GpuTexture, Mesh, ProxyId, RenderBackend, Screen};
use crate::scene::Scene;
use crate::simulation::WorldView;
use crate::texture::Texture;

/// A game run by the engine, headless: no window, no OS input. Events go
/// straight to the game, as game-level `InputEvent`s.
pub struct Headless<G: Game> {
    engine: Engine<G>,
    backend: NullBackend,
}

impl<G: Game> Headless<G> {
    /// Creates the game with `new_game` filling in its scene, and loads it,
    /// as `Engine::new` does.
    pub fn new(new_game: impl FnOnce(&mut Scene) -> G) -> Self {
        let mut backend = NullBackend::new(HEADLESS_SCREEN);
        let engine = Engine::new(&mut backend, new_game);
        Self { engine, backend }
    }

    /// Hands `event` to the game, as if the player did it.
    pub fn input(&mut self, event: InputEvent) -> Flow {
        self.engine.input_event(event)
    }

    /// One frame of `frame_time` seconds: physics steps, the game's logic,
    /// its view and HUD; drawn nowhere.
    pub fn frame(&mut self, frame_time: f64) {
        self.engine.frame(&mut self.backend, frame_time);
    }

    /// The world's data, read-only, as the game sees it.
    pub fn world(&self) -> WorldView<'_> {
        self.engine.world()
    }

    /// The game being run, to check its state.
    pub fn game(&self) -> &G {
        self.engine.game()
    }
}

/// The screen a headless engine pretends to draw on: 720p, standard
/// density.
const HEADLESS_SCREEN: Screen = Screen {
    width: 1280.0,
    height: 720.0,
    scale: 1.0,
};

/// A render backend that draws nothing, like Unreal's "null RHI" or
/// Godot's dummy renderer: the engine's rendering logic runs, nothing
/// reaches a GPU. It still hands out real IDs, from arenas holding
/// nothing.
pub(crate) struct NullBackend {
    screen: Screen,
    textures: Arena<GpuTexture, ()>,
    proxies: Arena<ProxyId, ()>,
}

impl NullBackend {
    pub(crate) fn new(screen: Screen) -> Self {
        Self {
            screen,
            textures: Arena::new(),
            proxies: Arena::new(),
        }
    }
}

impl RenderBackend for NullBackend {
    fn screen(&self) -> Screen {
        self.screen
    }
    fn set_background(&mut self, _color: Color) {}
    fn set_sun(&mut self, _direction: Vec3) {}
    fn set_camera(&mut self, _eye: Vec3, _target: Vec3) {}
    fn create_texture(&mut self, _texture: &Texture) -> GpuTexture {
        self.textures.insert(())
    }
    fn create_mesh(&mut self, _: &Mesh, _: Color, _: Option<GpuTexture>) -> ProxyId {
        self.proxies.insert(())
    }
    fn set_position(&mut self, _proxy: ProxyId, _position: Vec3) {}
    fn screen_text(&mut self, _text: &str, _at: Vec2, _size: f32, _color: Color) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::testing::TestGame;
    use crate::input::Key;

    #[test]
    fn a_headless_game_runs_the_whole_engine() {
        // One ball sliding north at 64 m/s, 1/64 s steps: 1 m per step.
        let mut headless = Headless::new(|scene| TestGame::spawn(scene, &[(0.0, 64.0)]));
        let ball = headless.game().focus;
        headless.frame(2.0 / 64.0);
        let north = headless.world().body(ball).map(|b| b.motion.position.x);
        assert_eq!(north, Some(2.0));
        assert_eq!(headless.game().steps_seen, 2);
    }

    #[test]
    fn events_go_straight_to_the_game() {
        let mut headless = Headless::new(|scene| TestGame::spawn(scene, &[(0.0, 0.0)]));
        assert_eq!(headless.input(InputEvent::KeyPressed(Key::Q)), Flow::Quit);
        assert_eq!(
            headless.game().last_event,
            Some(InputEvent::KeyPressed(Key::Q))
        );
        assert_eq!(headless.input(InputEvent::Scroll(10.0)), Flow::Continue);
    }
}
