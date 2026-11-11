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

use glam::{Vec2, Vec3};
use physics::{DVec2, DVec3};

use crate::camera::Camera;
use crate::scene::{Ground, GroundGrid, Scene};
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

pub(crate) const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

/// The geometry of a [`Visual`], in metres.
// TODO(later): `Mesh(asset)` for our own 3D models (glTF from Blender).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Sphere { radius: f32 },
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
/// World positions and directions are f32 metres (GPUs work in f32), in
/// **render axes**: x east, y up, z south (the y-up convention of graphics
/// libraries); `to_render` converts from the physics' NED.
pub trait RenderBackend {
    fn screen(&self) -> Screen;
    fn set_background(&mut self, color: Color);
    /// A directional light, like the sun, shining along `direction`.
    fn set_sun(&mut self, direction: Vec3);
    /// The camera: at `eye`, looking at `target`.
    fn set_camera(&mut self, eye: Vec3, target: Vec3);

    /// Retained objects: created once, then moved every frame.
    fn create_proxy(&mut self, visual: &Visual) -> ProxyId;
    fn set_position(&mut self, proxy: ProxyId, position: Vec3);

    /// Immediate drawing, re-issued every frame. Two coordinate spaces, as
    /// in Unreal (`DrawDebugLine` in the world, `UCanvas::DrawText` on the
    /// screen), named so every call says which one it uses:
    /// a debug line in **world** space (metres), `width` in pixels...
    fn world_line(&mut self, from: Vec3, to: Vec3, color: Color, width: f32);
    /// ...and text in **screen** space (physical pixels, top-left, y down).
    fn screen_text(&mut self, text: &str, at: Vec2, size: f32, color: Color);
}

/// The direction sunlight travels (NED): from the south-east, high in the
/// sky, so it goes north-west and down.
const SUN_DIRECTION: DVec3 = DVec3::new(0.4, -0.3, 0.85);

/// Width of the grid lines, in physical pixels.
const MAJOR_LINE_WIDTH: f32 = 2.0;
const MINOR_LINE_WIDTH: f32 = 1.0;

/// Draws the world: one render proxy per scene object, moved every frame
/// to where the simulation says, plus world-space debug drawing, all as
/// seen by a given camera. (The camera itself is a game-side object, as in
/// engines; the HUD is a separate overlay, see `hud`.)
pub(crate) struct Renderer {
    /// One render proxy per scene object, in the same order as the world's
    /// bodies: proxy `i` shows body `i`.
    // TODO(step-11): keyed by entity ID instead of index; created when an
    // entity spawns, freed when it despawns (like Godot's `free_rid`).
    proxies: Vec<ProxyId>,
    /// The scene's ground, drawn as a grid (none: nothing to draw).
    ground: Option<Ground>,
}

impl Renderer {
    /// Creates the scene's render proxies, its background and its light.
    pub(crate) fn new(backend: &mut impl RenderBackend, scene: &Scene) -> Self {
        backend.set_background(scene.background);
        backend.set_sun(to_render(SUN_DIRECTION.normalize()));
        Self {
            proxies: scene
                .objects
                .iter()
                .map(|object| backend.create_proxy(&object.visual))
                .collect(),
            ground: scene.ground,
        }
    }

    /// Prepares the world for this frame: camera, proxies, ground grid.
    pub(crate) fn draw(&self, backend: &mut impl RenderBackend, sim: &Simulation, camera: &Camera) {
        backend.set_camera(to_render(camera.eye()), to_render(camera.target));

        // ═══════════════════════════════════════════════════════════════
        //  PHYSICS -> SCENE
        //
        //  The only place where simulation state flows into graphics.
        //  Physics never knows the scene exists; once per frame the drawn
        //  (interpolated) positions are copied into the render proxies,
        //  in one loop over all objects.
        //  TODO(step-08): orientations too (position + rotation).
        // ═══════════════════════════════════════════════════════════════
        for (proxy, position) in self.proxies.iter().zip(sim.drawn_positions()) {
            backend.set_position(*proxy, to_render(position));
        }

        if let Some(ground) = &self.ground {
            let grid = &ground.grid;
            for line in grid_lines(camera.target.truncate(), grid, ground.elevation) {
                let (color, width) = if line.major {
                    (grid.major_color, MAJOR_LINE_WIDTH)
                } else {
                    (grid.minor_color, MINOR_LINE_WIDTH)
                };
                backend.world_line(to_render(line.from), to_render(line.to), color, width);
            }
        }
    }
}

/// One line of the ground grid (NED, on the ground).
#[derive(Debug)]
struct GridLine {
    from: DVec3,
    to: DVec3,
    major: bool,
}

/// The ground grid around `centre` (north, east, in metres), on a ground at
/// `elevation` metres: lines running east-west and north-south.
///
/// The grid's centre is snapped to whole major cells, so as the camera
/// moves the lines stay put on the ground instead of sliding along with
/// it. Returns an *iterator*: lines are produced as they're drawn, never
/// stored, so no memory is allocated per frame.
fn grid_lines(centre: DVec2, grid: &GroundGrid, elevation: f64) -> impl Iterator<Item = GridLine> {
    let grid = *grid; // a small `Copy` struct: the iterator owns its copy
    let major_every = i64::from(grid.major_every);
    let cell = grid.spacing * major_every as f64;
    let snapped = (centre / cell).round() * cell;
    let lines_each_side = (grid.half_size / grid.spacing) as i64;
    let (half, z) = (grid.half_size, -elevation); // z is down
    (-lines_each_side..=lines_each_side).flat_map(move |i| {
        let offset = i as f64 * grid.spacing;
        // rem_euclid, not %: -10 % 5 is 0 but -3 % 5 is -3 in Rust.
        let major = i.rem_euclid(major_every) == 0;
        let (north, east) = (snapped.x + offset, snapped.y + offset);
        let (south_edge, north_edge) = (snapped.x - half, snapped.x + half);
        let (west_edge, east_edge) = (snapped.y - half, snapped.y + half);
        [
            // Running east-west, at a fixed `north`.
            GridLine {
                from: DVec3::new(north, west_edge, z),
                to: DVec3::new(north, east_edge, z),
                major,
            },
            // Running north-south, at a fixed `east`.
            GridLine {
                from: DVec3::new(south_edge, east, z),
                to: DVec3::new(north_edge, east, z),
                major,
            },
        ]
    })
}

/// Physics position or direction (NED, f64 metres) -> render axes (f32,
/// what GPUs use): x east, y up, z south. This is the *only* place the two
/// axis conventions meet (see ROADMAP "Axes").
///
/// Both are right-handed, so the mapping is a pure rotation: lengths,
/// angles and cross products are preserved (checked by the tests).
// TODO(later): f32 only has ~7 significant digits: at 100 km from the
// origin, positions snap to ~1 cm steps and the picture jitters. The usual
// fix is a "floating origin": subtract the camera position in f64 *before*
// converting, so the GPU only sees small numbers.
fn to_render(ned: DVec3) -> Vec3 {
    let (north, east, down) = (ned.x, ned.y, ned.z);
    Vec3::new(east as f32, -down as f32, -north as f32)
}

/// Test doubles for `RenderBackend`, shared by tests in several modules.
#[cfg(test)]
pub(crate) mod fakes {
    use super::*;

    /// Records every call, so tests can check what was drawn.
    pub(crate) struct RecordingBackend {
        pub(crate) screen: Screen,
        pub(crate) background: Option<Color>,
        pub(crate) sun: Option<Vec3>,
        /// Latest camera: eye and target.
        pub(crate) camera: Option<(Vec3, Vec3)>,
        /// Created proxies: their visual and latest position.
        pub(crate) proxies: Vec<(Visual, Option<Vec3>)>,
        pub(crate) lines: Vec<(Vec3, Vec3)>,
        /// Text, position, size.
        pub(crate) texts: Vec<(String, Vec2, f32)>,
    }

    impl RecordingBackend {
        pub(crate) fn new(screen: Screen) -> Self {
            Self {
                screen,
                background: None,
                sun: None,
                camera: None,
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
        fn set_sun(&mut self, direction: Vec3) {
            self.sun = Some(direction);
        }
        fn set_camera(&mut self, eye: Vec3, target: Vec3) {
            self.camera = Some((eye, target));
        }
        fn create_proxy(&mut self, visual: &Visual) -> ProxyId {
            self.proxies.push((*visual, None));
            ProxyId(self.proxies.len() - 1)
        }
        fn set_position(&mut self, proxy: ProxyId, position: Vec3) {
            self.proxies[proxy.0].1 = Some(position);
        }
        fn world_line(&mut self, from: Vec3, to: Vec3, _color: Color, _width: f32) {
            self.lines.push((from, to));
        }
        fn screen_text(&mut self, text: &str, at: Vec2, size: f32, _color: Color) {
            self.texts.push((text.to_owned(), at, size));
        }
    }

    /// Ignores everything, like Unreal's "null RHI": lets the renderer's
    /// logic run without any graphics, e.g. to measure its allocations.
    pub(crate) struct NullBackend(pub(crate) Screen);

    impl RenderBackend for NullBackend {
        fn screen(&self) -> Screen {
            self.0
        }
        fn set_background(&mut self, _color: Color) {}
        fn set_sun(&mut self, _direction: Vec3) {}
        fn set_camera(&mut self, _eye: Vec3, _target: Vec3) {}
        fn create_proxy(&mut self, _visual: &Visual) -> ProxyId {
            ProxyId(0)
        }
        fn set_position(&mut self, _proxy: ProxyId, _position: Vec3) {}
        fn world_line(&mut self, _from: Vec3, _to: Vec3, _color: Color, _width: f32) {}
        fn screen_text(&mut self, _text: &str, _at: Vec2, _size: f32, _color: Color) {}
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::RecordingBackend;
    use super::*;
    use crate::scene::{Ground, GroundGrid, SceneObject};
    use physics::{Body, Motion, STANDARD_GRAVITY, Sphere};

    const BALL: Visual = Visual {
        shape: Shape::Sphere { radius: 0.4 },
        color: WHITE,
    };
    const BACKGROUND: Color = Color::rgb(0.1, 0.2, 0.3);

    const SCREEN: Screen = Screen {
        width: 1000.0,
        height: 600.0,
        scale: 1.0,
    };

    /// A ball resting on the ground at (north, east): it doesn't move, so
    /// drawn positions are exact.
    fn ball_at(north: f64, east: f64) -> SceneObject {
        SceneObject {
            body: Body {
                motion: Motion {
                    position: DVec3::new(north, east, -0.4),
                    velocity: DVec3::ZERO,
                },
                collider: Sphere { radius: 0.4 },
            },
            visual: BALL,
        }
    }

    /// Flat ground at sea level, default grid.
    const GROUND: Ground = Ground {
        elevation: 0.0,
        grid: GRID,
    };
    const GRID: GroundGrid = GroundGrid {
        spacing: 2.0,
        major_every: 5,
        half_size: 50.0,
        minor_color: Color::rgb(0.1, 0.1, 0.1),
        major_color: Color::rgb(0.5, 0.5, 0.5),
    };

    fn scene(objects: Vec<SceneObject>) -> Scene {
        Scene {
            objects,
            focus: 0,
            background: BACKGROUND,
            gravity: STANDARD_GRAVITY,
            ground: Some(GROUND),
        }
    }

    /// The scene's simulation, a renderer for it on `backend`, and a camera
    /// looking at `target`.
    fn setup(
        scene: &Scene,
        backend: &mut RecordingBackend,
        target: DVec3,
    ) -> (Simulation, Renderer, Camera) {
        let sim = Simulation::new(scene.world(), 1.0 / 64.0);
        (sim, Renderer::new(backend, scene), Camera::new(target))
    }

    #[test]
    fn ned_axes_map_to_render_axes() {
        assert_eq!(to_render(DVec3::X), Vec3::new(0.0, 0.0, -1.0)); // north: -z
        assert_eq!(to_render(DVec3::Y), Vec3::new(1.0, 0.0, 0.0)); // east: +x
        assert_eq!(to_render(DVec3::Z), Vec3::new(0.0, -1.0, 0.0)); // down: -y
    }

    #[test]
    fn the_conversion_keeps_handedness() {
        // A rotation, not a mirror: cross products survive the conversion,
        // so a body turning right in physics turns right on screen.
        let (n, e) = (DVec3::X, DVec3::Y);
        assert_eq!(to_render(n.cross(e)), to_render(n).cross(to_render(e)));
    }

    #[test]
    fn creates_one_proxy_per_object_with_background_and_sun() {
        let mut backend = RecordingBackend::new(SCREEN);
        setup(
            &scene(vec![ball_at(1.0, 0.0), ball_at(2.0, 0.0)]),
            &mut backend,
            DVec3::ZERO,
        );
        assert_eq!(backend.proxies.len(), 2);
        assert_eq!(backend.proxies[0].0, BALL);
        assert_eq!(backend.background, Some(BACKGROUND));
        let sun = backend.sun.expect("a sun is set");
        assert!(sun.y < 0.0, "sunlight goes down: {sun}");
        assert!((sun.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn each_proxy_is_placed_where_the_simulation_says() {
        let mut backend = RecordingBackend::new(SCREEN);
        let objects = vec![ball_at(1.0, 2.0), ball_at(-3.0, 4.0)];
        let (sim, renderer, camera) = setup(&scene(objects), &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        // NED (north, east, -0.4) -> render (east, 0.4, -north).
        let positions: Vec<_> = backend.proxies.iter().map(|(_, p)| *p).collect();
        assert_eq!(
            positions,
            [
                Some(Vec3::new(2.0, 0.4, -1.0)),
                Some(Vec3::new(4.0, 0.4, 3.0))
            ]
        );
    }

    #[test]
    fn proxies_are_created_once_not_every_frame() {
        // Retained mode: the proxy lives across frames; only its position
        // is updated.
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) =
            setup(&scene(vec![ball_at(0.0, 0.0)]), &mut backend, DVec3::ZERO);
        for _ in 0..3 {
            renderer.draw(&mut backend, &sim, &camera);
        }
        assert_eq!(backend.proxies.len(), 1);
    }

    #[test]
    fn camera_is_sent_in_render_axes() {
        let mut backend = RecordingBackend::new(SCREEN);
        let target = DVec3::new(40.0, 12.0, -5.0);
        let (sim, renderer, camera) = setup(&scene(vec![ball_at(0.0, 0.0)]), &mut backend, target);
        renderer.draw(&mut backend, &sim, &camera);
        let (eye, sent_target) = backend.camera.expect("a camera is set");
        assert_eq!(sent_target, to_render(target));
        assert_eq!(eye, to_render(camera.eye()));
    }

    #[test]
    fn the_grid_lies_on_the_ground_around_the_camera_target() {
        let mut backend = RecordingBackend::new(SCREEN);
        let target = DVec3::new(100.0, -40.0, -5.0);
        let (sim, renderer, camera) = setup(&scene(vec![ball_at(0.0, 0.0)]), &mut backend, target);
        renderer.draw(&mut backend, &sim, &camera);
        // 51 lines each way, all on the ground (render y = 0), spanning
        // 50 m either side of the target.
        assert_eq!(backend.lines.len(), 2 * 51);
        assert!(backend.lines.iter().all(|(a, b)| a.y == 0.0 && b.y == 0.0));
        let easts = backend.lines.iter().flat_map(|(a, b)| [a.x, b.x]);
        let (min, max) = easts.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert_eq!((min, max), (-90.0, 10.0));
    }

    #[test]
    fn the_grid_is_drawn_at_the_ground_elevation_with_the_scene_look() {
        let mut plateau = scene(vec![ball_at(0.0, 0.0)]);
        plateau.ground = Some(Ground {
            elevation: 7.0,
            grid: GroundGrid {
                spacing: 10.0,
                half_size: 20.0,
                ..GRID
            },
        });
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&plateau, &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        // Render y is up: the grid floats 7 m up; 5 lines each way.
        assert!(backend.lines.iter().all(|(a, b)| a.y == 7.0 && b.y == 7.0));
        assert_eq!(backend.lines.len(), 2 * 5);
    }

    #[test]
    fn without_a_ground_there_is_no_grid() {
        let mut empty_sky = scene(vec![ball_at(0.0, 0.0)]);
        empty_sky.ground = None;
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&empty_sky, &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        assert!(backend.lines.is_empty());
    }

    #[test]
    fn the_grid_stays_fixed_to_the_ground_as_the_camera_moves() {
        let lines = |centre| {
            grid_lines(centre, &GRID, 0.0)
                .map(|l| (l.from, l.to))
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(DVec2::new(1.0, 2.0)), lines(DVec2::new(3.0, -4.0)));
    }

    #[test]
    fn every_fifth_grid_line_is_a_major_one() {
        let majors: Vec<f64> = grid_lines(DVec2::ZERO, &GRID, 0.0)
            // East-west lines: their north coordinate doesn't change.
            .filter(|line| line.major && line.from.x == line.to.x)
            .map(|line| line.from.x)
            .collect();
        // East-west lines every 10 m (5 x 2 m), from 50 m south to 50 m north.
        assert_eq!(
            majors,
            [
                -50.0, -40.0, -30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0, 40.0, 50.0
            ]
        );
    }

    #[test]
    fn colors_are_opaque_unless_said_otherwise() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3).a, 1.0);
    }
}
