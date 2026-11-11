//! Rendering: every decision about *how the world is drawn*, written
//! against a small generic interface (`RenderBackend`) rather than a
//! graphics library, so it can be tested.
//!
//! The layering follows established engines (Godot's `RenderingServer`,
//! Unreal's RHI and scene proxies, Bevy's render world):
//! - **`RenderBackend`**: a handful of generic operations. Create a render
//!   proxy (a sphere), move it, set the camera, draw debug lines and text.
//!   It knows geometry, not scene objects: a ground's [`Visual`] becomes
//!   lines, a ball's a sphere. It grows only with new kinds of geometry
//!   (meshes, later), never with content: a plane or a building will be a
//!   mesh (data), not a new function.
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

pub(crate) const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

/// The geometry of a [`Visual`], in metres.
// TODO(later): `Mesh(asset)` for our own 3D models (glTF from Blender).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Sphere {
        radius: f32,
    },
    /// A ground drawn as a grid of lines, on the horizontal plane through
    /// its object's position.
    Grid(GroundGrid),
}

/// What an object looks like: data, not code. Engines call this a mesh +
/// material (Unity's MeshRenderer, Godot's MeshInstance, Bevy's `Mesh` +
/// `Material` components). The physics never sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub shape: Shape,
    /// The object's colour (for a grid: its major lines).
    pub color: Color,
}

/// How a ground grid looks: a line every `spacing` metres, a brighter one
/// (the visual's colour) every `major_every` lines, over `half_size`
/// metres around what the camera looks at.
// TODO(later): textured terrain, from real elevation data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    /// Metres between lines.
    pub spacing: f64,
    pub major_every: u32,
    /// Metres drawn on each side of the camera's target.
    pub half_size: f64,
    pub minor_color: Color,
}

impl Default for GroundGrid {
    /// A line every 2 m, a major one every 10 m, 50 m around the camera.
    fn default() -> Self {
        Self {
            spacing: 2.0,
            major_every: 5,
            half_size: 50.0,
            minor_color: Color::rgb(0.08, 0.08, 0.08),
        }
    }
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

    /// Retained objects: created once, then moved every frame. Here a
    /// sphere of `radius` metres.
    // TODO(later): `create_mesh` for our own 3D models.
    fn create_sphere(&mut self, radius: f32, color: Color) -> ProxyId;
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

/// How one scene object is drawn.
enum Drawable {
    /// A retained render proxy, moved every frame.
    Proxy(ProxyId),
    /// A ground grid, re-drawn every frame around the camera.
    Grid {
        grid: GroundGrid,
        major_color: Color,
    },
}

/// Draws the world, every scene object according to its visual, as seen by
/// a given camera. (The camera itself is a game-side object, as in engines;
/// the HUD is a separate overlay, see `hud`.)
pub(crate) struct Renderer {
    /// One per scene object, in scene order: drawable `i` shows body `i`.
    // TODO(step-11): keyed by entity ID instead of index; created when an
    // entity spawns, freed when it despawns (like Godot's `free_rid`).
    drawables: Vec<Drawable>,
}

impl Renderer {
    /// Creates the scene's render proxies, its background and its light.
    pub(crate) fn new(backend: &mut impl RenderBackend, scene: &Scene) -> Self {
        backend.set_background(scene.background);
        backend.set_sun(to_render(SUN_DIRECTION.normalize()));
        let drawables = scene
            .objects
            .iter()
            .map(|object| match object.visual.shape {
                Shape::Sphere { radius } => {
                    Drawable::Proxy(backend.create_sphere(radius, object.visual.color))
                }
                Shape::Grid(grid) => Drawable::Grid {
                    grid,
                    major_color: object.visual.color,
                },
            })
            .collect();
        Self { drawables }
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
        for (body, drawable) in self.drawables.iter().enumerate() {
            let position = sim.drawn_position(body);
            match drawable {
                Drawable::Proxy(proxy) => backend.set_position(*proxy, to_render(position)),
                Drawable::Grid { grid, major_color } => {
                    // Under the camera's target, at the object's altitude.
                    for line in grid_lines(camera.target.truncate(), grid, -position.z) {
                        let (color, width) = if line.major {
                            (*major_color, MAJOR_LINE_WIDTH)
                        } else {
                            (grid.minor_color, MINOR_LINE_WIDTH)
                        };
                        backend.world_line(to_render(line.from), to_render(line.to), color, width);
                    }
                }
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
        /// Created spheres: radius, colour and latest position.
        pub(crate) proxies: Vec<(f32, Color, Option<Vec3>)>,
        /// World lines: ends and colour.
        pub(crate) lines: Vec<(Vec3, Vec3, Color)>,
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
        fn create_sphere(&mut self, radius: f32, color: Color) -> ProxyId {
            self.proxies.push((radius, color, None));
            ProxyId(self.proxies.len() - 1)
        }
        fn set_position(&mut self, proxy: ProxyId, position: Vec3) {
            self.proxies[proxy.0].2 = Some(position);
        }
        fn world_line(&mut self, from: Vec3, to: Vec3, color: Color, _width: f32) {
            self.lines.push((from, to, color));
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
        fn create_sphere(&mut self, _radius: f32, _color: Color) -> ProxyId {
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
    use crate::scene::SceneObject;
    use physics::{Body, BodyType, Collider, Motion, STANDARD_GRAVITY};

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
                body_type: BodyType::Dynamic,
                motion: Motion {
                    position: DVec3::new(north, east, -0.4),
                    velocity: DVec3::ZERO,
                },
                collider: Collider::Sphere { radius: 0.4 },
            },
            visual: BALL,
        }
    }

    const GRID: GroundGrid = GroundGrid {
        spacing: 2.0,
        major_every: 5,
        half_size: 50.0,
        minor_color: MINOR,
    };
    const MINOR: Color = Color::rgb(0.1, 0.1, 0.1);
    const MAJOR: Color = Color::rgb(0.5, 0.5, 0.5);

    /// Flat ground at `elevation` metres, drawn as `grid`.
    fn ground(elevation: f64, grid: GroundGrid) -> SceneObject {
        SceneObject {
            body: Body {
                body_type: BodyType::Static,
                motion: Motion {
                    position: DVec3::new(0.0, 0.0, -elevation),
                    velocity: DVec3::ZERO,
                },
                collider: Collider::HalfSpace,
            },
            visual: Visual {
                shape: Shape::Grid(grid),
                color: MAJOR,
            },
        }
    }

    /// A scene of `objects`, focused on the first one.
    fn scene(objects: Vec<SceneObject>) -> Scene {
        Scene {
            objects,
            focus: 0,
            background: BACKGROUND,
            gravity: STANDARD_GRAVITY,
        }
    }

    /// A ground at sea level, then a ball at the origin.
    fn ball_on_the_ground() -> Scene {
        scene(vec![ground(0.0, GRID), ball_at(0.0, 0.0)])
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
    fn creates_one_proxy_per_ball_with_background_and_sun() {
        // The ground is drawn as lines every frame: no proxy.
        let mut backend = RecordingBackend::new(SCREEN);
        setup(
            &scene(vec![
                ground(0.0, GRID),
                ball_at(1.0, 0.0),
                ball_at(2.0, 0.0),
            ]),
            &mut backend,
            DVec3::ZERO,
        );
        assert_eq!(backend.proxies.len(), 2);
        assert_eq!(backend.proxies[0], (0.4, WHITE, None));
        assert_eq!(backend.background, Some(BACKGROUND));
        let sun = backend.sun.expect("a sun is set");
        assert!(sun.y < 0.0, "sunlight goes down: {sun}");
        assert!((sun.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn each_proxy_is_placed_where_the_simulation_says() {
        let mut backend = RecordingBackend::new(SCREEN);
        // The ground in between, to check each proxy follows its own body.
        let objects = vec![ball_at(1.0, 2.0), ground(0.0, GRID), ball_at(-3.0, 4.0)];
        let (sim, renderer, camera) = setup(&scene(objects), &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        // NED (north, east, -0.4) -> render (east, 0.4, -north).
        let positions: Vec<_> = backend.proxies.iter().map(|(_, _, p)| *p).collect();
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
        let (sim, renderer, camera) = setup(&ball_on_the_ground(), &mut backend, DVec3::ZERO);
        for _ in 0..3 {
            renderer.draw(&mut backend, &sim, &camera);
        }
        assert_eq!(backend.proxies.len(), 1);
    }

    #[test]
    fn camera_is_sent_in_render_axes() {
        let mut backend = RecordingBackend::new(SCREEN);
        let target = DVec3::new(40.0, 12.0, -5.0);
        let (sim, renderer, camera) = setup(&ball_on_the_ground(), &mut backend, target);
        renderer.draw(&mut backend, &sim, &camera);
        let (eye, sent_target) = backend.camera.expect("a camera is set");
        assert_eq!(sent_target, to_render(target));
        assert_eq!(eye, to_render(camera.eye()));
    }

    #[test]
    fn the_grid_lies_on_the_ground_around_the_camera_target() {
        let mut backend = RecordingBackend::new(SCREEN);
        let target = DVec3::new(100.0, -40.0, -5.0);
        let (sim, renderer, camera) = setup(&ball_on_the_ground(), &mut backend, target);
        renderer.draw(&mut backend, &sim, &camera);
        // 51 lines each way, all on the ground (render y = 0), spanning
        // 50 m either side of the target.
        assert_eq!(backend.lines.len(), 2 * 51);
        assert!(
            backend
                .lines
                .iter()
                .all(|(a, b, _)| a.y == 0.0 && b.y == 0.0)
        );
        let easts = backend.lines.iter().flat_map(|(a, b, _)| [a.x, b.x]);
        let (min, max) = easts.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert_eq!((min, max), (-90.0, 10.0));
    }

    #[test]
    fn the_grid_is_drawn_at_the_ground_elevation_with_its_own_look() {
        let grid = GroundGrid {
            spacing: 10.0,
            half_size: 20.0,
            ..GRID
        };
        let plateau = scene(vec![ball_at(0.0, 0.0), ground(7.0, grid)]);
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&plateau, &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        // Render y is up: the grid floats 7 m up; 5 lines each way.
        assert!(
            backend
                .lines
                .iter()
                .all(|(a, b, _)| a.y == 7.0 && b.y == 7.0)
        );
        assert_eq!(backend.lines.len(), 2 * 5);
    }

    #[test]
    fn the_visual_colour_draws_major_lines_the_grid_minor_colour_the_others() {
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer, camera) = setup(&ball_on_the_ground(), &mut backend, DVec3::ZERO);
        renderer.draw(&mut backend, &sim, &camera);
        let count = |color| backend.lines.iter().filter(|l| l.2 == color).count();
        // 11 major and 40 minor lines each way (see `every_fifth_...`).
        assert_eq!((count(MAJOR), count(MINOR)), (2 * 11, 2 * 40));
    }

    #[test]
    fn without_a_ground_there_is_no_grid() {
        let empty_sky = scene(vec![ball_at(0.0, 0.0)]);
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
