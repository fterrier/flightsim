//! Rendering: every decision about *how the world is drawn*, written
//! against a small generic interface (`RenderBackend`) rather than a
//! graphics library, so it can be tested.
//!
//! The layering follows established engines (Godot's `RenderingServer`,
//! Unreal's RHI and scene proxies, Bevy's render world):
//! - **`RenderBackend`**: a handful of generic operations. Create a render
//!   proxy from a [`Visual`], move it, set the camera, draw debug lines and
//!   text. It never grows with content: a plane or a building will be a
//!   new `Visual` (data), not a new function.
//! - **`Kiss3dBackend`** (`kiss3d_backend.rs`) implements it with one
//!   kiss3d call per method: a "humble object", too simple to hide bugs,
//!   and the only rendering code tests can't run.
//! - **`Renderer`** (here) makes every drawing *decision*: which proxy goes
//!   where, f64 -> f32, zoom limits, ruler and HUD layout. Tests run it
//!   against a fake backend that records calls (the idea behind Unreal's
//!   "null RHI").

use glam::Vec2;
use physics::DVec2;

use crate::hud::{self, Hud};
use crate::simulation::RenderState;
use crate::view;

/// A colour, in *linear* light (how much light, not how bright it looks).
/// Backends convert to sRGB, what screens expect, which brightens dark
/// values a lot: linear 0.01 shows as about 10% grey, not 1%.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// An opaque colour.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

pub const GREY: Color = Color::rgb(0.5, 0.5, 0.5);
pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

/// The geometry of a [`Visual`], in metres.
// TODO(step-04): add `Sphere`, and boxes for a ground grid.
// TODO(later): `Mesh(asset)` for our own 3D models (glTF from Blender).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle { radius: f32 },
}

/// What an object looks like: data, not code. Engines call this a mesh +
/// material (Unity's MeshRenderer, Godot's MeshInstance, Bevy's `Mesh` +
/// `Material` components). The physics never sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub shape: Shape,
    pub color: Color,
}

/// A handle to a render proxy owned by the backend (like Godot's `RID`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxyId(pub usize);

/// The output surface, in *physical* pixels. `scale` is the HiDPI factor:
/// physical pixels per logical pixel (1 standard, 2 Retina).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

/// The few generic operations rendering needs from a graphics library.
/// Positions are f32 world-space metres (GPUs work in f32); the HUD's text
/// positions are in screen space (physical pixels, top-left, y down).
pub trait RenderBackend {
    fn screen(&self) -> Screen;
    /// Current camera zoom, physical pixels per metre (the mouse wheel may
    /// have changed it).
    fn zoom(&self) -> f32;
    fn set_camera(&mut self, at: Vec2, zoom: f32);

    /// Retained objects: created once, then moved every frame.
    fn create_proxy(&mut self, visual: &Visual) -> ProxyId;
    fn set_position(&mut self, proxy: ProxyId, position: Vec2);

    /// Immediate debug drawing, re-issued every frame (Unreal's
    /// `DrawDebugLine`, Bevy's gizmos).
    fn line(&mut self, from: Vec2, to: Vec2, color: Color, width: f32);
    fn text(&mut self, text: &str, at: Vec2, size: f32, color: Color);
}

/// Width of the ground and ruler lines, in physical pixels.
const LINE_WIDTH: f32 = 2.0;

pub struct Renderer {
    /// The ball's render proxy.
    // TODO(step-11): one proxy per entity that has a `Visual`: created
    // when the entity spawns, freed when it despawns (like Godot's
    // `free_rid`), all updated in a single loop in `draw_frame`.
    ball: ProxyId,
}

impl Renderer {
    /// Creates the render proxies and the starting camera.
    pub fn new(backend: &mut impl RenderBackend, ball: &Visual) -> Self {
        let scale = backend.screen().scale;
        backend.set_camera(Vec2::ZERO, view::initial_zoom(scale));
        Self {
            ball: backend.create_proxy(ball),
        }
    }

    /// Metres visible above and below the camera centre, for framing.
    pub fn visible_half_height(&self, backend: &impl RenderBackend) -> f64 {
        let screen = backend.screen();
        let zoom = view::clamp_zoom(backend.zoom(), screen.scale);
        view::visible_half_height(screen.height, zoom)
    }

    /// Prepares one frame: camera, proxies, ruler, HUD. `drawn` is the
    /// (interpolated) state to show, `look_at` where the camera points.
    pub fn draw_frame(
        &self,
        backend: &mut impl RenderBackend,
        drawn: &RenderState,
        look_at: DVec2,
        hud: &Hud,
    ) {
        let screen = backend.screen();
        // The mouse wheel may have zoomed the camera; keep it in range.
        let zoom = view::clamp_zoom(backend.zoom(), screen.scale);
        backend.set_camera(to_render(look_at), zoom);

        // ═══════════════════════════════════════════════════════════════
        //  PHYSICS -> SCENE
        //
        //  The only place where simulation state flows into graphics.
        //  Physics never knows the scene exists; once per frame the drawn
        //  (interpolated) state is copied into the render proxies. In 3D
        //  it stays the same: position + orientation -> scene node.
        // ═══════════════════════════════════════════════════════════════
        backend.set_position(self.ball, to_render(drawn.ball));

        // Ground line and distance ruler, in world space.
        let (from, to) = view::visible_x_range(look_at.x, screen.width, zoom);
        let ground = |x: f64, y: f64| to_render(DVec2::new(x, y));
        backend.line(ground(from, 0.0), ground(to, 0.0), GREY, LINE_WIDTH);
        for tick in view::ruler_ticks(from, to) {
            let (top, bottom) = (ground(tick.x, 0.0), ground(tick.x, -tick.length));
            backend.line(top, bottom, GREY, LINE_WIDTH);
        }

        // HUD, in screen space.
        for (i, line) in hud.lines().enumerate() {
            let (x, y) = hud::line_position(i, screen.scale);
            backend.text(line, Vec2::new(x, y), hud::font_size(screen.scale), WHITE);
        }
    }
}

/// Physics position (f64 metres) -> render position (f32 metres, what
/// GPUs use).
// TODO(step-04): physics moves to 3D NED axes (z down); convert to the
// renderer's y-up axes here, and only here.
// TODO(later): f32 only has ~7 significant digits: at 100 km from the
// origin, positions snap to ~1 cm steps and the picture jitters. The usual
// fix is a "floating origin": subtract the camera position in f64 *before*
// converting, so the GPU only sees small numbers.
fn to_render(position: DVec2) -> Vec2 {
    position.as_vec2()
}

/// Test doubles for `RenderBackend`, shared by tests in several modules.
#[cfg(test)]
pub mod fakes {
    use super::*;

    /// Records every call, so tests can check what was drawn.
    pub struct RecordingBackend {
        pub screen: Screen,
        /// Camera zoom (physical px per metre), as the mouse wheel left it.
        pub zoom: f32,
        pub camera_at: Option<Vec2>,
        /// Created proxies: their visual and latest position.
        pub proxies: Vec<(Visual, Option<Vec2>)>,
        pub lines: Vec<(Vec2, Vec2)>,
        /// Text, position, size.
        pub texts: Vec<(String, Vec2, f32)>,
    }

    impl RecordingBackend {
        pub fn new(screen: Screen) -> Self {
            Self {
                screen,
                zoom: 1.0,
                camera_at: None,
                proxies: Vec::new(),
                lines: Vec::new(),
                texts: Vec::new(),
            }
        }
    }

    impl RenderBackend for RecordingBackend {
        fn screen(&self) -> Screen {
            self.screen
        }
        fn zoom(&self) -> f32 {
            self.zoom
        }
        fn set_camera(&mut self, at: Vec2, zoom: f32) {
            self.camera_at = Some(at);
            self.zoom = zoom;
        }
        fn create_proxy(&mut self, visual: &Visual) -> ProxyId {
            self.proxies.push((*visual, None));
            ProxyId(self.proxies.len() - 1)
        }
        fn set_position(&mut self, proxy: ProxyId, position: Vec2) {
            self.proxies[proxy.0].1 = Some(position);
        }
        fn line(&mut self, from: Vec2, to: Vec2, _color: Color, _width: f32) {
            self.lines.push((from, to));
        }
        fn text(&mut self, text: &str, at: Vec2, size: f32, _color: Color) {
            self.texts.push((text.to_owned(), at, size));
        }
    }

    /// Ignores everything, like Unreal's "null RHI": lets the renderer's
    /// logic run without any graphics, e.g. to measure its allocations.
    pub struct NullBackend(pub Screen);

    impl RenderBackend for NullBackend {
        fn screen(&self) -> Screen {
            self.0
        }
        fn zoom(&self) -> f32 {
            20.0
        }
        fn set_camera(&mut self, _at: Vec2, _zoom: f32) {}
        fn create_proxy(&mut self, _visual: &Visual) -> ProxyId {
            ProxyId(0)
        }
        fn set_position(&mut self, _proxy: ProxyId, _position: Vec2) {}
        fn line(&mut self, _from: Vec2, _to: Vec2, _color: Color, _width: f32) {}
        fn text(&mut self, _text: &str, _at: Vec2, _size: f32, _color: Color) {}
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::RecordingBackend;
    use super::*;
    use crate::camera::CameraMode;
    use crate::hud::FrameStats;
    use crate::simulation::Simulation;
    use physics::{Body, Circle, World};

    const BALL: Visual = Visual {
        shape: Shape::Circle { radius: 0.4 },
        color: WHITE,
    };

    /// A 1000 x 600 px standard-density screen.
    const SCREEN: Screen = Screen {
        width: 1000.0,
        height: 600.0,
        scale: 1.0,
    };

    fn drawn_at(x: f64, y: f64) -> RenderState {
        RenderState {
            ball: DVec2::new(x, y),
        }
    }

    /// A HUD with some text in it.
    fn hud() -> Hud {
        let ball = Body {
            position: DVec2::new(0.0, 0.4),
            velocity: DVec2::ZERO,
        };
        let sim = Simulation::new(World::new(ball, Circle { radius: 0.4 }), 1.0 / 64.0);
        let mut hud = Hud::default();
        hud.update(&sim, 0, &FrameStats::default(), CameraMode::Follow);
        hud
    }

    #[test]
    fn creates_one_proxy_from_the_visual_and_sets_the_starting_zoom() {
        let mut backend = RecordingBackend::new(SCREEN);
        Renderer::new(&mut backend, &BALL);
        assert_eq!(backend.proxies.len(), 1);
        assert_eq!(backend.proxies[0].0, BALL);
        assert_eq!(backend.zoom, view::initial_zoom(1.0));
    }

    #[test]
    fn places_the_proxy_at_the_drawn_position() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        renderer.draw_frame(&mut backend, &drawn_at(3.5, 2.25), DVec2::ZERO, &hud());
        assert_eq!(backend.proxies[0].1, Some(Vec2::new(3.5, 2.25)));
    }

    #[test]
    fn proxies_are_created_once_not_every_frame() {
        // Retained mode: the proxy lives across frames; only its position
        // is updated.
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        for x in 0..3 {
            renderer.draw_frame(&mut backend, &drawn_at(x as f64, 1.0), DVec2::ZERO, &hud());
        }
        assert_eq!(backend.proxies.len(), 1);
        assert_eq!(backend.proxies[0].1, Some(Vec2::new(2.0, 1.0)));
    }

    #[test]
    fn camera_looks_where_it_is_told() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        let look_at = DVec2::new(40.0, 12.0);
        renderer.draw_frame(&mut backend, &drawn_at(0.0, 0.0), look_at, &hud());
        assert_eq!(backend.camera_at, Some(Vec2::new(40.0, 12.0)));
    }

    #[test]
    fn zoom_from_the_mouse_wheel_is_kept_within_limits() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        backend.zoom = 10_000.0; // scrolled far in
        renderer.draw_frame(&mut backend, &drawn_at(0.0, 0.0), DVec2::ZERO, &hud());
        assert_eq!(backend.zoom, view::clamp_zoom(10_000.0, 1.0));
        assert!(backend.zoom < 10_000.0);
    }

    #[test]
    fn visible_half_height_uses_the_clamped_zoom() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        backend.zoom = 10_000.0;
        let clamped = view::clamp_zoom(10_000.0, 1.0);
        assert_eq!(
            renderer.visible_half_height(&backend),
            view::visible_half_height(600.0, clamped)
        );
    }

    #[test]
    fn ground_line_and_ruler_span_the_visible_range() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        backend.zoom = 20.0; // 1000 px / 20 px/m = 50 m across
        let look_at = DVec2::new(100.0, 10.0);
        renderer.draw_frame(&mut backend, &drawn_at(0.0, 0.0), look_at, &hud());
        // The ground: one line across the visible range, at y = 0.
        assert_eq!(
            backend.lines[0],
            (Vec2::new(75.0, 0.0), Vec2::new(125.0, 0.0))
        );
        // Then one tick per whole metre from 75 to 125 m.
        assert_eq!(backend.lines.len(), 1 + 51);
        assert_eq!(backend.lines[1].0, Vec2::new(75.0, 0.0));
    }

    #[test]
    fn every_hud_line_is_drawn_in_order() {
        let mut backend = RecordingBackend::new(SCREEN);
        let renderer = Renderer::new(&mut backend, &BALL);
        let hud = hud();
        renderer.draw_frame(&mut backend, &drawn_at(0.0, 0.0), DVec2::ZERO, &hud);
        let drawn: Vec<&str> = backend.texts.iter().map(|(t, _, _)| t.as_str()).collect();
        let expected: Vec<&str> = hud.lines().collect();
        assert_eq!(drawn, expected);
        let (x, y) = hud::line_position(1, 1.0);
        assert_eq!(backend.texts[1].1, Vec2::new(x, y));
    }

    #[test]
    fn hud_text_is_twice_as_big_on_a_retina_display() {
        let retina = Screen {
            width: 2000.0,
            height: 1200.0,
            scale: 2.0,
        };
        let mut normal = RecordingBackend::new(SCREEN);
        let mut double = RecordingBackend::new(retina);
        let (a, b) = (
            Renderer::new(&mut normal, &BALL),
            Renderer::new(&mut double, &BALL),
        );
        a.draw_frame(&mut normal, &drawn_at(0.0, 0.0), DVec2::ZERO, &hud());
        b.draw_frame(&mut double, &drawn_at(0.0, 0.0), DVec2::ZERO, &hud());
        assert_eq!(double.texts[0].2, 2.0 * normal.texts[0].2);
    }

    #[test]
    fn colors_are_opaque_unless_said_otherwise() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3).a, 1.0);
    }
}
