//! The engine's platform layer on kiss3d: opens the window, runs the game
//! loop (clock, input events, frames), and implements [`RenderBackend`]
//! with kiss3d. Like Bevy's `bevy_winit` runner: the engine core decides,
//! this crate talks to the operating system and the GPU.
//!
//! Everything here is a "humble object": no decisions, only translation
//! between the engine's types and kiss3d's, so there is nothing worth unit
//! testing, and it needs a window and a GPU, which CI doesn't have. The
//! whole crate is excluded from coverage; any logic belongs in `engine`.

use std::sync::Arc;
use std::thread;
use std::time::Instant;

use engine::{
    Color, Engine, Flow, Game, InputEvent, Key, ProxyId, RenderBackend, Screen, Shape, Visual,
};
use glam::Vec2;
use kiss3d::camera::PanZoomCamera2d;
use kiss3d::color::Color as Kiss3dColor;
use kiss3d::event::{Action, Key as Kiss3dKey, WindowEvent};
use kiss3d::glamx::Vec2 as Kiss3dVec2;
use kiss3d::scene::SceneNode2d;
use kiss3d::text::Font;
use kiss3d::window::Window;

/// Runs `game` in a window titled `title` until it quits or the window is
/// closed.
pub fn run(title: &str, game: impl Game) {
    // kiss3d's API is `async` so the same code can run in a browser, where
    // the browser owns the loop. On desktop we simply block on it: the
    // loop below really is ours, on the main thread.
    kiss3d::pollster::block_on(run_async(title, game));
}

async fn run_async(title: &str, game: impl Game) {
    let mut backend = Kiss3dBackend::new(title, 1280, 720).await;
    let mut engine = Engine::new(&mut backend, game);
    let mut last_frame: Option<Instant> = None;

    // The game loop: one iteration per displayed frame. `present` shows
    // the frame prepared by the previous iteration.
    while backend.present().await {
        // Clock: how much real time did the last frame take? The first
        // frame also contains window and GPU setup (~150 ms measured),
        // which must not be simulated, so the clock starts at 0 once the
        // first frame is on screen.
        let now = Instant::now();
        let frame_time = last_frame.map_or(0.0, |last| (now - last).as_secs_f64());
        last_frame = Some(now);

        // Input: kiss3d events, translated into the engine's own.
        for event in backend.window.events().iter() {
            if let Some(input) = input_event(event.value)
                && engine.input(input) == Flow::Quit
            {
                return;
            }
        }

        // Debug lag (see `engine::Engine::debug_delay`).
        thread::sleep(engine.debug_delay());

        engine.frame(&mut backend, frame_time);
    }
}

/// `RenderBackend` with kiss3d. Private: the crate's only API is `run`.
struct Kiss3dBackend {
    window: Window,
    /// Objects kiss3d keeps between frames (retained mode).
    scene: SceneNode2d,
    /// Projects world space (metres, y up) onto the screen, as told by
    /// `set_camera`.
    camera: PanZoomCamera2d,
    /// Render proxies, indexed by `ProxyId`.
    proxies: Vec<SceneNode2d>,
    font: Arc<Font>,
}

impl Kiss3dBackend {
    /// Opens the OS window and sets up the GPU (wgpu: Metal on macOS,
    /// Vulkan/DX12 elsewhere).
    async fn new(title: &str, width: u32, height: u32) -> Self {
        let window = Window::new_with_size(title, width, height).await;
        // Our `Camera` steers and zooms: turn off kiss3d's own mouse
        // handling (drag to pan; a zoom step of 1 makes the wheel a no-op).
        let mut camera = PanZoomCamera2d::new(Kiss3dVec2::ZERO, 1.0);
        camera.rebind_drag_button(None);
        camera.set_zoom_step(1.0);
        Self {
            window,
            scene: SceneNode2d::empty(),
            camera,
            proxies: Vec::new(),
            font: Font::default(),
        }
    }

    /// Draws the frame prepared since the last call and processes OS events
    /// (keys, mouse wheel, resize). Returns `false` once the window closes.
    async fn present(&mut self) -> bool {
        self.window
            .render_2d(&mut self.scene, &mut self.camera)
            .await
    }
}

impl RenderBackend for Kiss3dBackend {
    fn screen(&self) -> Screen {
        Screen {
            width: self.window.width() as f32,
            height: self.window.height() as f32,
            scale: self.window.scale_factor() as f32,
        }
    }

    fn set_background(&mut self, color: Color) {
        self.window.set_background_color(kiss3d_color(color));
    }

    fn set_camera(&mut self, at: Vec2, zoom: f32) {
        self.camera.set_at(kiss3d_vec(at));
        self.camera.set_zoom(zoom);
    }

    fn create_proxy(&mut self, visual: &Visual) -> ProxyId {
        let mut node = match visual.shape {
            Shape::Circle { radius } => self.scene.add_circle(radius),
        };
        node.set_color(kiss3d_color(visual.color));
        self.proxies.push(node);
        ProxyId(self.proxies.len() - 1)
    }

    fn set_position(&mut self, proxy: ProxyId, position: Vec2) {
        self.proxies[proxy.0].set_position(kiss3d_vec(position));
    }

    fn world_line(&mut self, from: Vec2, to: Vec2, color: Color, width: f32) {
        let (from, to) = (kiss3d_vec(from), kiss3d_vec(to));
        self.window
            .draw_line_2d(from, to, kiss3d_color(color), width);
    }

    fn screen_text(&mut self, text: &str, at: Vec2, size: f32, color: Color) {
        let color = kiss3d_color(color);
        self.window
            .draw_text(text, kiss3d_vec(at), size, &self.font, color);
    }
}

/// Same data, different crates: our `glam` vector vs the (older) `glam`
/// version kiss3d uses internally.
fn kiss3d_vec(v: Vec2) -> Kiss3dVec2 {
    Kiss3dVec2::new(v.x, v.y)
}

fn kiss3d_color(c: Color) -> Kiss3dColor {
    Kiss3dColor::new(c.r, c.g, c.b, c.a)
}

/// A kiss3d window event as an engine input event, if the engine cares.
fn input_event(event: WindowEvent) -> Option<InputEvent> {
    match event {
        WindowEvent::Key(key, Action::Press, _) => engine_key(key).map(InputEvent::KeyPressed),
        WindowEvent::Scroll(_, scroll, _) => Some(InputEvent::Scroll(scroll)),
        _ => None,
    }
}

/// A kiss3d key as an engine key, for the keys the engine knows.
#[rustfmt::skip]
fn engine_key(key: Kiss3dKey) -> Option<Key> {
    use Kiss3dKey as K;
    Some(match key {
        K::A => Key::A, K::B => Key::B, K::C => Key::C, K::D => Key::D,
        K::E => Key::E, K::F => Key::F, K::G => Key::G, K::H => Key::H,
        K::I => Key::I, K::J => Key::J, K::K => Key::K, K::L => Key::L,
        K::M => Key::M, K::N => Key::N, K::O => Key::O, K::P => Key::P,
        K::Q => Key::Q, K::R => Key::R, K::S => Key::S, K::T => Key::T,
        K::U => Key::U, K::V => Key::V, K::W => Key::W, K::X => Key::X,
        K::Y => Key::Y, K::Z => Key::Z,
        K::Escape => Key::Escape, K::Space => Key::Space,
        K::Up => Key::Up, K::Down => Key::Down, K::Left => Key::Left, K::Right => Key::Right,
        _ => return None,
    })
}
