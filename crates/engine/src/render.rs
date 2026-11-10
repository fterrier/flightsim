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
//! - **`Kiss3dBackend`** (crate `engine_kiss3d`) implements it with one
//!   kiss3d call per method: a "humble object", too simple to hide bugs,
//!   and the only rendering code tests can't run.
//! - **`Renderer`** (here) makes the world's drawing *decisions*: which
//!   proxy goes where, f64 -> f32, what the camera sends, debug drawing.
//!   Tests run it against a fake backend that records calls (the idea
//!   behind Unreal's "null RHI").

use glam::Vec2;
use physics::DVec2;

use crate::camera::Camera;
use crate::scene::Scene;
use crate::simulation::Simulation;

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
/// World positions are f32 metres (GPUs work in f32).
pub trait RenderBackend {
    fn screen(&self) -> Screen;
    fn set_background(&mut self, color: Color);
    /// Where the camera looks (world space) and its zoom, in *physical*
    /// pixels per metre. The backend only applies it: the camera's state
    /// lives in our `Camera`.
    fn set_camera(&mut self, at: Vec2, zoom: f32);

    /// Retained objects: created once, then moved every frame.
    fn create_proxy(&mut self, visual: &Visual) -> ProxyId;
    fn set_position(&mut self, proxy: ProxyId, position: Vec2);

    /// Immediate drawing, re-issued every frame. Two coordinate spaces, as
    /// in Unreal (`DrawDebugLine` in the world, `UCanvas::DrawText` on the
    /// screen), named so every call says which one it uses:
    /// a debug line in **world** space (metres), `width` in pixels...
    fn world_line(&mut self, from: Vec2, to: Vec2, color: Color, width: f32);
    /// ...and text in **screen** space (physical pixels, top-left, y down).
    fn screen_text(&mut self, text: &str, at: Vec2, size: f32, color: Color);
}

/// Width of the ground and ruler lines, in physical pixels.
const LINE_WIDTH: f32 = 2.0;

/// Ruler tick lengths, in metres (they're world-space objects).
const MAJOR_TICK: f64 = 1.0;
const MINOR_TICK: f64 = 0.4;

/// Draws the world: one render proxy per scene object, moved every frame
/// to where the simulation says, plus world-space debug drawing, all as
/// seen by a given camera. (The camera itself is a game-side object, as in
/// engines; the HUD is a separate overlay, see `hud`.)
pub struct Renderer {
    /// One render proxy per scene object, in the same order as the world's
    /// bodies: proxy `i` shows body `i`.
    // TODO(step-11): keyed by entity ID instead of index; created when an
    // entity spawns, freed when it despawns (like Godot's `free_rid`).
    proxies: Vec<ProxyId>,
}

impl Renderer {
    /// Creates the scene's render proxies and sets its background.
    pub fn new(backend: &mut impl RenderBackend, scene: &Scene) -> Self {
        backend.set_background(scene.background);
        Self {
            proxies: scene
                .objects
                .iter()
                .map(|object| backend.create_proxy(&object.visual))
                .collect(),
        }
    }

    /// Prepares the world for this frame: camera, proxies, debug drawing.
    pub fn draw(&self, backend: &mut impl RenderBackend, sim: &Simulation, camera: &Camera) {
        let screen = backend.screen();
        // The camera thinks in logical pixels; the backend in physical ones.
        let look_at = camera.position;
        backend.set_camera(to_render(look_at), camera.zoom() * screen.scale);

        // ═══════════════════════════════════════════════════════════════
        //  PHYSICS -> SCENE
        //
        //  The only place where simulation state flows into graphics.
        //  Physics never knows the scene exists; once per frame the drawn
        //  (interpolated) positions are copied into the render proxies,
        //  in one loop over all objects. In 3D it stays the same:
        //  position + orientation -> scene node.
        // ═══════════════════════════════════════════════════════════════
        for (proxy, position) in self.proxies.iter().zip(sim.drawn_positions()) {
            backend.set_position(*proxy, to_render(position));
        }

        // Ground line and distance ruler, in world space, across what the
        // camera sees.
        // TODO(step-04): replaced by a 3D ground grid.
        let half_width = camera.visible_half_extents(viewport(screen)).x;
        let (from, to) = (look_at.x - half_width, look_at.x + half_width);
        let ground = |x: f64, y: f64| to_render(DVec2::new(x, y));
        backend.world_line(ground(from, 0.0), ground(to, 0.0), GREY, LINE_WIDTH);
        for tick in ruler_ticks(from, to) {
            let (top, bottom) = (ground(tick.x, 0.0), ground(tick.x, -tick.length));
            backend.world_line(top, bottom, GREY, LINE_WIDTH);
        }
    }
}

/// The screen's size in logical pixels (what the camera works with).
pub fn viewport(screen: Screen) -> DVec2 {
    DVec2::new(screen.width as f64, screen.height as f64) / screen.scale as f64
}

/// One mark of the distance ruler drawn along the ground (world space).
#[derive(Debug)]
struct Tick {
    /// Position along the ground, in metres.
    x: f64,
    /// How far the mark extends below the ground, in metres.
    length: f64,
}

/// One tick per whole metre between `from` and `to` (metres), longer
/// every 10 m. Only the visible part of the ruler is generated.
///
/// Returns an *iterator*: ticks are produced one by one as they're drawn,
/// never stored, so no memory is allocated per frame (a `Vec` would be
/// allocated and freed every frame).
fn ruler_ticks(from: f64, to: f64) -> impl Iterator<Item = Tick> {
    (from.ceil() as i64..=to.floor() as i64).map(|m| Tick {
        x: m as f64,
        // rem_euclid, not %: -10 % 10 is 0 but -3 % 10 is -3 in Rust.
        length: if m.rem_euclid(10) == 0 {
            MAJOR_TICK
        } else {
            MINOR_TICK
        },
    })
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
        pub background: Option<Color>,
        /// Latest camera zoom, physical px per metre.
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
                background: None,
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
        fn set_background(&mut self, color: Color) {
            self.background = Some(color);
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
        fn world_line(&mut self, from: Vec2, to: Vec2, _color: Color, _width: f32) {
            self.lines.push((from, to));
        }
        fn screen_text(&mut self, text: &str, at: Vec2, size: f32, _color: Color) {
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
        fn set_background(&mut self, _color: Color) {}
        fn set_camera(&mut self, _at: Vec2, _zoom: f32) {}
        fn create_proxy(&mut self, _visual: &Visual) -> ProxyId {
            ProxyId(0)
        }
        fn set_position(&mut self, _proxy: ProxyId, _position: Vec2) {}
        fn world_line(&mut self, _from: Vec2, _to: Vec2, _color: Color, _width: f32) {}
        fn screen_text(&mut self, _text: &str, _at: Vec2, _size: f32, _color: Color) {}
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::RecordingBackend;
    use super::*;
    use crate::camera;
    use crate::scene::SceneObject;
    use physics::{Body, Circle, Motion, STANDARD_GRAVITY};

    const BALL: Visual = Visual {
        shape: Shape::Circle { radius: 0.4 },
        color: WHITE,
    };
    const BACKGROUND: Color = Color::rgb(0.1, 0.2, 0.3);

    /// A 1000 x 600 px standard-density screen.
    const SCREEN: Screen = Screen {
        width: 1000.0,
        height: 600.0,
        scale: 1.0,
    };

    /// A ball resting on the ground at `x` metres: it doesn't move, so
    /// drawn positions are exact.
    fn ball_at(x: f64) -> SceneObject {
        SceneObject {
            body: Body {
                motion: Motion {
                    position: DVec2::new(x, 0.4),
                    velocity: DVec2::ZERO,
                },
                collider: Circle { radius: 0.4 },
            },
            visual: BALL,
        }
    }

    fn balls_at(xs: &[f64]) -> Scene {
        Scene {
            objects: xs.iter().map(|&x| ball_at(x)).collect(),
            focus: 0,
            background: BACKGROUND,
            gravity: STANDARD_GRAVITY,
        }
    }

    /// The scene's simulation, a renderer for it on `backend`, and a camera
    /// looking at `look_at`.
    fn setup(
        scene: &Scene,
        backend: &mut RecordingBackend,
        look_at: DVec2,
    ) -> (Simulation, Renderer, Camera) {
        let sim = Simulation::new(scene.world(), 1.0 / 64.0);
        (sim, Renderer::new(backend, scene), Camera::new(look_at))
    }

    #[test]
    fn creates_one_proxy_per_object_and_sets_the_background() {
        let mut backend = RecordingBackend::new(SCREEN);
        setup(&balls_at(&[1.0, 2.0]), &mut backend, DVec2::ZERO);
        assert_eq!(backend.proxies.len(), 2);
        assert_eq!(backend.proxies[0].0, BALL);
        assert_eq!(backend.background, Some(BACKGROUND));
    }

    #[test]
    fn each_proxy_is_placed_where_the_simulation_says() {
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&balls_at(&[1.0, 2.0, 3.0]), &mut backend, DVec2::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        let positions: Vec<_> = backend.proxies.iter().map(|(_, p)| *p).collect();
        assert_eq!(positions, [1.0, 2.0, 3.0].map(|x| Some(Vec2::new(x, 0.4))));
    }

    #[test]
    fn proxies_are_created_once_not_every_frame() {
        // Retained mode: the proxy lives across frames; only its position
        // is updated.
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&balls_at(&[0.0]), &mut backend, DVec2::ZERO);
        for _ in 0..3 {
            renderer.draw(&mut backend, &sim, &camera);
        }
        assert_eq!(backend.proxies.len(), 1);
    }

    #[test]
    fn camera_is_sent_where_it_looks_with_its_zoom_in_physical_pixels() {
        // The camera thinks in logical pixels; the backend gets physical
        // ones: twice as many per metre on a Retina display.
        let retina = Screen {
            width: 2000.0,
            height: 1200.0,
            scale: 2.0,
        };
        let mut backend = RecordingBackend::new(retina);
        let look_at = DVec2::new(40.0, 12.0);
        let (sim, renderer, camera) = setup(&balls_at(&[0.0]), &mut backend, look_at);
        renderer.draw(&mut backend, &sim, &camera);
        assert_eq!(backend.camera_at, Some(Vec2::new(40.0, 12.0)));
        assert_eq!(backend.zoom, 2.0 * camera::DEFAULT_ZOOM);
    }

    #[test]
    fn ground_line_and_ruler_span_the_visible_range() {
        // 1500 px wide at 15 px/m = 100 m across, centred on x = 100 m.
        let wide = Screen {
            width: 1500.0,
            height: 600.0,
            scale: 1.0,
        };
        let mut backend = RecordingBackend::new(wide);
        let look_at = DVec2::new(100.0, 10.0);
        let (sim, renderer, camera) = setup(&balls_at(&[0.0]), &mut backend, look_at);
        renderer.draw(&mut backend, &sim, &camera);
        // The ground: one line across the visible range, at y = 0.
        assert_eq!(
            backend.lines[0],
            (Vec2::new(50.0, 0.0), Vec2::new(150.0, 0.0))
        );
        // Then one tick per whole metre from 50 to 150 m.
        assert_eq!(backend.lines.len(), 1 + 101);
        assert_eq!(backend.lines[1].0, Vec2::new(50.0, 0.0));
    }

    #[test]
    fn ruler_has_a_tick_on_every_whole_metre_in_range() {
        let xs: Vec<f64> = ruler_ticks(0.5, 3.2).map(|t| t.x).collect();
        assert_eq!(xs, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn every_tenth_tick_is_longer_including_negative_ones() {
        let ticks: Vec<Tick> = ruler_ticks(-10.0, 10.0).collect();
        assert_eq!(ticks.len(), 21);
        let long: Vec<f64> = ticks
            .iter()
            .filter(|t| t.length == MAJOR_TICK)
            .map(|t| t.x)
            .collect();
        assert_eq!(long, [-10.0, 0.0, 10.0]);
        const { assert!(MINOR_TICK < MAJOR_TICK) } // checked at compile time
    }

    #[test]
    fn colors_are_opaque_unless_said_otherwise() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3).a, 1.0);
    }
}
