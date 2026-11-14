//! The engine's platform layer on kiss3d: opens the window, runs the game
//! loop (clock, input events, frames), and implements [`RenderBackend`]
//! with kiss3d. Like Bevy's `bevy_winit` runner: the engine core decides,
//! this crate talks to the operating system and the GPU.
//!
//! Everything here is a "humble object": no decisions, only translation
//! between the engine's types and kiss3d's, so there is nothing worth unit
//! testing, and it needs a window and a GPU, which CI doesn't have. The
//! whole crate is excluded from coverage; any logic belongs in `engine`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use arena::Arena;
use engine::platform::{
    Engine, GpuTexture, Mesh, ProxyId, RawInput, RenderBackend, Screen, VIEW_DISTANCE,
};
use engine::{Color, DVec2, Flow, Game, Key, MouseButton, Scene, Texture};
use glam::{Vec2, Vec3};
use kiss3d::camera::OrbitCamera3d;
use kiss3d::color::Color as Kiss3dColor;
use kiss3d::event::{Action, Key as Kiss3dKey, MouseButton as Kiss3dButton, WindowEvent};
use kiss3d::glamx::{Vec2 as Kiss3dVec2, Vec3 as Kiss3dVec3};
use kiss3d::light::Light;
use kiss3d::resource::{GpuMesh3d, Texture as Kiss3dTexture};
use kiss3d::scene::SceneNode3d;
use kiss3d::text::Font;
use kiss3d::window::Window;

/// Runs a game in a window titled `title` until it quits or the window is
/// closed. `new_game` creates it, filling in its scene (see `Engine::new`).
pub fn run<G: Game>(title: &str, new_game: impl FnOnce(&mut Scene) -> G) {
    // kiss3d's API is `async` so the same code can run in a browser, where
    // the browser owns the loop. On desktop we simply block on it: the
    // loop below really is ours, on the main thread.
    kiss3d::pollster::block_on(run_async(title, new_game));
}

async fn run_async<G: Game>(title: &str, new_game: impl FnOnce(&mut Scene) -> G) {
    let mut backend = Kiss3dBackend::new(title, 1280, 720).await;
    let mut engine = Engine::new(&mut backend, new_game);
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
            if let Some(input) = raw_input(event.value)
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
    scene: SceneNode3d,
    /// Projects the world (render axes: y up) onto the screen, placed by
    /// `set_camera`.
    camera: OrbitCamera3d,
    /// Render proxies, by the IDs handed out for them.
    proxies: Arena<ProxyId, SceneNode3d>,
    /// Uploaded textures, by the IDs handed out for them.
    textures: Arena<GpuTexture, Arc<Kiss3dTexture>>,
    font: Arc<Font>,
}

impl Kiss3dBackend {
    /// Opens the OS window and sets up the GPU (wgpu: Metal on macOS,
    /// Vulkan/DX12 elsewhere).
    async fn new(title: &str, width: u32, height: u32) -> Self {
        let window = Window::new_with_size(title, width, height).await;
        // Our `Camera` decides where to look from: turn off kiss3d's own
        // camera controls (mouse rotate and drag, reset key; a zoom step of
        // 1 makes the wheel a no-op) and its limits, which would otherwise
        // override what `set_camera` asks for.
        let (eye, at) = (Kiss3dVec3::new(0.0, 10.0, 40.0), Kiss3dVec3::ZERO);
        let fov = std::f32::consts::FRAC_PI_4;
        let mut camera = OrbitCamera3d::new_with_frustum(fov, 0.1, VIEW_DISTANCE, eye, at);
        camera.rebind_rotate_button(None);
        camera.rebind_drag_button(None);
        camera.rebind_reset_key(None);
        camera.set_dist_step(1.0);
        camera.set_min_dist(0.0);
        camera.set_max_dist(f32::MAX);
        camera.set_min_pitch(0.0);
        camera.set_max_pitch(std::f32::consts::PI);
        Self {
            window,
            scene: SceneNode3d::empty(),
            camera,
            proxies: Arena::new(),
            textures: Arena::new(),
            font: Font::default(),
        }
    }

    /// Draws the frame prepared since the last call and processes OS events
    /// (keys, mouse wheel, resize). Returns `false` once the window closes.
    async fn present(&mut self) -> bool {
        self.window
            .render_3d(&mut self.scene, &mut self.camera)
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

    fn set_sun(&mut self, direction: Vec3) {
        let sun = Light::directional(kiss3d_vec3(direction)).with_intensity(2.0);
        self.scene.add_light(sun);
    }

    fn set_camera(&mut self, eye: Vec3, target: Vec3) {
        self.camera.look_at(kiss3d_vec3(eye), kiss3d_vec3(target));
    }

    fn set_position(&mut self, proxy: ProxyId, position: Vec3) {
        if let Some(node) = self.proxies.get_mut(proxy) {
            node.set_position(kiss3d_vec3(position));
        }
    }

    fn create_texture(&mut self, texture: &Texture) -> GpuTexture {
        // Repeated, smoothly filtered, with mipmaps and 16x anisotropic
        // filtering: sharp up close, no shimmer far away or at grazing
        // angles.
        self.textures.insert(Kiss3dTexture::new(
            texture.width,
            texture.height,
            &texture.srgba,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Linear,
            true,
            16,
        ))
    }

    fn create_mesh(&mut self, mesh: &Mesh, color: Color, texture: Option<GpuTexture>) -> ProxyId {
        let gpu_mesh = GpuMesh3d::new(
            mesh.positions.iter().copied().map(kiss3d_vec3).collect(),
            mesh.triangles.clone(),
            Some(mesh.normals.iter().copied().map(kiss3d_vec3).collect()),
            Some(mesh.uvs.iter().copied().map(kiss3d_vec).collect()),
            false,
        );
        let mut node = self
            .scene
            .add_mesh(Rc::new(RefCell::new(gpu_mesh)), Kiss3dVec3::ONE);
        node.set_color(kiss3d_color(color));
        if let Some(texture) = texture.and_then(|id| self.textures.get(id)) {
            node.set_texture(texture.clone());
        }
        self.proxies.insert(node)
    }

    fn screen_text(&mut self, text: &str, at: Vec2, size: f32, color: Color) {
        let color = kiss3d_color(color);
        self.window
            .draw_text(text, kiss3d_vec(at), size, &self.font, color);
    }
}

/// Same data, different crates: our `glam` vectors vs the (older) `glam`
/// version kiss3d uses internally.
fn kiss3d_vec(v: Vec2) -> Kiss3dVec2 {
    Kiss3dVec2::new(v.x, v.y)
}

fn kiss3d_vec3(v: Vec3) -> Kiss3dVec3 {
    Kiss3dVec3::new(v.x, v.y, v.z)
}

fn kiss3d_color(c: Color) -> Kiss3dColor {
    Kiss3dColor::new(c.r, c.g, c.b, c.a)
}

/// A kiss3d window event as raw engine input, if the engine cares.
fn raw_input(event: WindowEvent) -> Option<RawInput> {
    match event {
        WindowEvent::Key(key, Action::Press, _) => engine_key(key).map(RawInput::KeyPressed),
        WindowEvent::Key(key, Action::Release, _) => engine_key(key).map(RawInput::KeyReleased),
        WindowEvent::Scroll(_, scroll, _) => Some(RawInput::Scroll(scroll)),
        WindowEvent::MouseButton(button, action, _) => Some(RawInput::MouseButton {
            button: engine_button(button)?,
            pressed: action == Action::Press,
        }),
        WindowEvent::CursorPos(x, y, _) => Some(RawInput::CursorMoved(DVec2::new(x, y))),
        _ => None,
    }
}

/// A kiss3d mouse button as an engine one, for the buttons the engine knows.
fn engine_button(button: Kiss3dButton) -> Option<MouseButton> {
    match button {
        Kiss3dButton::Button1 => Some(MouseButton::Left),
        Kiss3dButton::Button2 => Some(MouseButton::Right),
        Kiss3dButton::Button3 => Some(MouseButton::Middle),
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
