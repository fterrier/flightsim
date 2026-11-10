//! [`RenderBackend`] implemented with kiss3d: one kiss3d call per method.
//!
//! A "humble object": no decisions, only translation from our types to
//! kiss3d's, so there is nothing here worth unit testing. It also needs a
//! window and a GPU, which CI doesn't have. Like `main.rs`, it's excluded
//! from coverage, so any logic belongs in `render.rs`, not here.

use std::sync::Arc;

use glam::Vec2;
use kiss3d::camera::PanZoomCamera2d;
use kiss3d::color::Color as Kiss3dColor;
use kiss3d::glamx::Vec2 as Kiss3dVec2;
use kiss3d::scene::SceneNode2d;
use kiss3d::text::Font;
use kiss3d::window::Window;

use crate::render::{Color, ProxyId, RenderBackend, Screen, Shape, Visual};

pub struct Kiss3dBackend {
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
    pub async fn new(title: &str, width: u32, height: u32, background: Color) -> Self {
        let mut window = Window::new_with_size(title, width, height).await;
        window.set_background_color(kiss3d_color(background));
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
    pub async fn present(&mut self) -> bool {
        self.window
            .render_2d(&mut self.scene, &mut self.camera)
            .await
    }

    /// kiss3d's window also delivers input (keys, events).
    pub fn window(&self) -> &Window {
        &self.window
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

    fn line(&mut self, from: Vec2, to: Vec2, color: Color, width: f32) {
        let (from, to) = (kiss3d_vec(from), kiss3d_vec(to));
        self.window
            .draw_line_2d(from, to, kiss3d_color(color), width);
    }

    fn text(&mut self, text: &str, at: Vec2, size: f32, color: Color) {
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
