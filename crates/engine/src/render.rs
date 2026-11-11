//! Rendering: every decision about *how the world is drawn*, written
//! against a small generic interface (`RenderBackend`) rather than a
//! graphics library, so it can be tested.
//!
//! The layering follows established engines (Godot's `RenderingServer`,
//! Unreal's RHI and scene proxies, Bevy's render world):
//! - **`RenderBackend`**: a handful of generic operations. Create a render
//!   proxy (a sphere, or a textured mesh), move it, set the camera, draw
//!   text. It knows geometry, not scene objects: each object's [`Visual`]
//!   becomes one proxy. It grows only with new kinds of geometry, never
//!   with content: a plane or a building will be a mesh (data), not a new
//!   function.
//! - **`Kiss3dBackend`** (crate `engine_kiss3d`) implements it with one
//!   kiss3d call per method: a "humble object", too simple to hide bugs,
//!   and the only rendering code tests can't run.
//! - **`Renderer`** (here) makes the world's drawing *decisions*: what
//!   each visual becomes, which proxy goes where, f64 -> f32, what the
//!   camera sends. Tests run it against a fake backend that records calls
//!   (the idea behind Unreal's "null RHI").

use glam::{Vec2, Vec3};
use physics::DVec3;

use crate::ground::{grid_texture, ground_mesh};
use crate::scene::Scene;
use crate::simulation::WorldView;

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
// TODO(later): `Mesh(asset)` for our own 3D models (glTF from Blender),
// drawn with `RenderBackend::create_mesh` like the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Sphere {
        radius: f32,
    },
    /// An endless flat ground through its object's position, covered with
    /// a grid: the look of a `HalfSpace` collider, endless too.
    GroundPlane(GroundGrid),
}

/// What an object looks like: data, not code. Engines call this a mesh +
/// material (Unity's MeshRenderer, Godot's MeshInstance, Bevy's `Mesh` +
/// `Material` components). The physics never sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub shape: Shape,
    /// The object's colour (for a ground plane: between the grid lines).
    pub color: Color,
}

/// How a ground plane looks: a line every `spacing` metres, a brighter one
/// every `major_every` lines.
// TODO(later): textured terrain, from real elevation data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    /// Metres between lines.
    pub spacing: f64,
    pub major_every: u32,
    pub minor_color: Color,
    pub major_color: Color,
}

impl GroundGrid {
    /// Metres covered by one major cell: the grid repeats with this
    /// period.
    pub(crate) fn major_spacing(&self) -> f64 {
        self.spacing * f64::from(self.major_every)
    }
}

impl Default for GroundGrid {
    /// A line every 2 m, a major one every 10 m.
    fn default() -> Self {
        Self {
            spacing: 2.0,
            major_every: 5,
            minor_color: Color::rgb(0.08, 0.08, 0.08),
            major_color: Color::rgb(0.5, 0.5, 0.5),
        }
    }
}

/// Where the camera is and what it looks at, in world space (NED, metres):
/// what the game decides every frame (`Game::view`), and the renderer
/// draws from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: DVec3,
    pub target: DVec3,
}

/// How far the camera sees, in metres: the backend's far clipping plane.
/// An endless ground is drawn this far around the camera, so it looks
/// endless wherever you are.
pub const VIEW_DISTANCE: f32 = 5000.0;

/// A handle to a render proxy owned by the backend (like Godot's `RID`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxyId(pub usize);

/// A triangle mesh, in render axes (f32 metres, x east, y up, z south),
/// around its proxy's position: what GPUs draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// One per position: the direction the surface faces there, for
    /// lighting.
    pub normals: Vec<Vec3>,
    /// One per position: where it falls on the texture, in texture widths.
    /// Beyond 0..1, the texture repeats.
    pub uvs: Vec<Vec2>,
    /// Indices into `positions`, counter-clockwise seen from the front.
    pub triangles: Vec<[u32; 3]>,
}

/// An image to cover a mesh with: `width` × `height` pixels, row by row
/// from the top-left, 4 bytes each (red, green, blue: sRGB-encoded, as
/// colour textures are stored; then alpha).
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub srgba: Vec<u8>,
}

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

    /// Retained objects: created once, then moved every frame. A sphere of
    /// `radius` metres...
    fn create_sphere(&mut self, radius: f32, color: Color) -> ProxyId;
    /// ...or any mesh, covered with `texture` (repeated, and filtered so it
    /// doesn't shimmer far away).
    fn create_mesh(&mut self, mesh: &Mesh, texture: &Texture) -> ProxyId;
    fn set_position(&mut self, proxy: ProxyId, position: Vec3);

    /// Immediate drawing, re-issued every frame: text in **screen** space
    /// (physical pixels, top-left, y down), like Unreal's
    /// `UCanvas::DrawText`.
    fn screen_text(&mut self, text: &str, at: Vec2, size: f32, color: Color);
}

/// The direction sunlight travels (NED): from the south-east, high in the
/// sky, so it goes north-west and down.
const SUN_DIRECTION: DVec3 = DVec3::new(0.4, -0.3, 0.85);

/// Where a proxy goes each frame.
#[derive(Clone, Copy, Debug)]
enum Placement {
    /// Where its body is.
    AtBody,
    /// At its body's altitude, but under the camera: the visible part of
    /// something endless. Moved in whole `tile`s (metres), so its texture
    /// stays fixed to the world.
    UnderCamera { tile: f64 },
}

/// Draws the world, every scene object according to its visual, from the
/// view the game chose. (How the camera moves is the game's business, as
/// in engines; the HUD is a separate overlay, see `hud`.)
pub(crate) struct Renderer {
    /// One render proxy per scene object, in scene order: proxy `i` shows
    /// body `i`.
    // TODO(step-11): keyed by entity ID instead of index; created when an
    // entity spawns, freed when it despawns (like Godot's `free_rid`).
    proxies: Vec<(ProxyId, Placement)>,
}

impl Renderer {
    /// Creates the scene's render proxies, its background and its light.
    pub(crate) fn new(backend: &mut impl RenderBackend, scene: &Scene) -> Self {
        backend.set_background(scene.background);
        backend.set_sun(to_render(SUN_DIRECTION.normalize()));
        let proxies = scene
            .objects
            .iter()
            .map(|object| match object.visual.shape {
                Shape::Sphere { radius } => (
                    backend.create_sphere(radius, object.visual.color),
                    Placement::AtBody,
                ),
                Shape::GroundPlane(grid) => {
                    let texture = grid_texture(&grid, object.visual.color);
                    let proxy = backend.create_mesh(&ground_mesh(&grid), &texture);
                    let tile = grid.major_spacing();
                    (proxy, Placement::UnderCamera { tile })
                }
            })
            .collect();
        Self { proxies }
    }

    /// Prepares the world for this frame: camera and proxies.
    pub(crate) fn draw(&self, backend: &mut impl RenderBackend, world: WorldView<'_>, view: View) {
        backend.set_camera(to_render(view.eye), to_render(view.target));

        // ═══════════════════════════════════════════════════════════════
        //  PHYSICS -> SCENE
        //
        //  The only place where simulation state flows into graphics.
        //  Physics never knows the scene exists; once per frame the drawn
        //  (interpolated) positions are copied into the render proxies,
        //  in one loop over all objects.
        //  TODO(step-08): orientations too (position + rotation).
        // ═══════════════════════════════════════════════════════════════
        for ((proxy, placement), &position) in self.proxies.iter().zip(world.drawn) {
            let position = match *placement {
                Placement::AtBody => position,
                Placement::UnderCamera { tile } => {
                    let under_eye = (view.eye.truncate() / tile).round() * tile;
                    under_eye.extend(position.z)
                }
            };
            backend.set_position(*proxy, to_render(position));
        }
    }
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

    /// A proxy, as the backend was asked to create it.
    #[derive(Debug, PartialEq)]
    pub(crate) enum Created {
        Sphere { radius: f32, color: Color },
        Mesh { mesh: Mesh, texture: Texture },
    }

    /// Records every call, so tests can check what was drawn.
    pub(crate) struct RecordingBackend {
        pub(crate) screen: Screen,
        pub(crate) background: Option<Color>,
        pub(crate) sun: Option<Vec3>,
        /// Latest camera: eye and target.
        pub(crate) camera: Option<(Vec3, Vec3)>,
        /// Created proxies, and their latest position.
        pub(crate) proxies: Vec<(Created, Option<Vec3>)>,
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
                texts: Vec::new(),
            }
        }

        fn create(&mut self, created: Created) -> ProxyId {
            self.proxies.push((created, None));
            ProxyId(self.proxies.len() - 1)
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
            self.create(Created::Sphere { radius, color })
        }
        fn create_mesh(&mut self, mesh: &Mesh, texture: &Texture) -> ProxyId {
            self.create(Created::Mesh {
                mesh: mesh.clone(),
                texture: texture.clone(),
            })
        }
        fn set_position(&mut self, proxy: ProxyId, position: Vec3) {
            self.proxies[proxy.0].1 = Some(position);
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
        fn create_mesh(&mut self, _mesh: &Mesh, _texture: &Texture) -> ProxyId {
            ProxyId(0)
        }
        fn set_position(&mut self, _proxy: ProxyId, _position: Vec3) {}
        fn screen_text(&mut self, _text: &str, _at: Vec2, _size: f32, _color: Color) {}
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::{Created, RecordingBackend};
    use super::*;
    use crate::scene::SceneObject;
    use crate::simulation::Simulation;
    use physics::{Body, BodyType, Collider, Motion, STANDARD_GRAVITY};

    const BALL: Visual = Visual {
        shape: Shape::Sphere { radius: 0.4 },
        color: WHITE,
    };
    const BACKGROUND: Color = Color::rgb(0.1, 0.2, 0.3);
    const GROUND_COLOR: Color = Color::rgb(0.02, 0.03, 0.02);

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

    /// Flat ground at `elevation` metres, drawn as the default grid.
    fn ground(elevation: f64) -> SceneObject {
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
                shape: Shape::GroundPlane(GroundGrid::default()),
                color: GROUND_COLOR,
            },
        }
    }

    fn scene(objects: Vec<SceneObject>) -> Scene {
        Scene {
            objects,
            background: BACKGROUND,
            gravity: STANDARD_GRAVITY,
        }
    }

    /// A ground at sea level, then a ball at the origin.
    fn ball_on_the_ground() -> Scene {
        scene(vec![ground(0.0), ball_at(0.0, 0.0)])
    }

    /// The scene's simulation, and a renderer for it on `backend`.
    fn setup(scene: &Scene, backend: &mut RecordingBackend) -> (Simulation, Renderer) {
        let sim = Simulation::new(scene.world().expect("a valid scene"), 1.0 / 64.0);
        (sim, Renderer::new(backend, scene))
    }

    /// Looking at the origin from `eye`.
    fn from(eye: DVec3) -> View {
        View {
            eye,
            target: DVec3::ZERO,
        }
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
        setup(&ball_on_the_ground(), &mut backend);
        assert_eq!(backend.proxies.len(), 2);
        assert_eq!(
            backend.proxies[1].0,
            Created::Sphere {
                radius: 0.4,
                color: WHITE
            }
        );
        assert_eq!(backend.background, Some(BACKGROUND));
        let sun = backend.sun.expect("a sun is set");
        assert!(sun.y < 0.0, "sunlight goes down: {sun}");
        assert!((sun.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_ground_plane_becomes_its_grid_mesh_and_texture() {
        let mut backend = RecordingBackend::new(SCREEN);
        setup(&ball_on_the_ground(), &mut backend);
        let grid = GroundGrid::default();
        assert_eq!(
            backend.proxies[0].0,
            Created::Mesh {
                mesh: ground_mesh(&grid),
                texture: grid_texture(&grid, GROUND_COLOR),
            }
        );
    }

    #[test]
    fn each_ball_is_placed_where_its_body_is() {
        let mut backend = RecordingBackend::new(SCREEN);
        // The ground in between, to check each proxy follows its own body.
        let objects = vec![ball_at(1.0, 2.0), ground(0.0), ball_at(-3.0, 4.0)];
        let (sim, renderer) = setup(&scene(objects), &mut backend);
        renderer.draw(&mut backend, sim.view(), from(DVec3::new(0.0, 0.0, -10.0)));
        // NED (north, east, -0.4) -> render (east, 0.4, -north).
        assert_eq!(backend.proxies[0].1, Some(Vec3::new(2.0, 0.4, -1.0)));
        assert_eq!(backend.proxies[2].1, Some(Vec3::new(4.0, 0.4, 3.0)));
    }

    /// Where the ground plane of a scene with a ground `elevation` metres
    /// up is drawn, seen from `eye`.
    fn ground_drawn_at(elevation: f64, eye: DVec3) -> Vec3 {
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer) = setup(&scene(vec![ground(elevation)]), &mut backend);
        renderer.draw(&mut backend, sim.view(), from(eye));
        backend.proxies[0].1.expect("the ground is placed")
    }

    #[test]
    fn the_endless_ground_is_drawn_under_the_camera_at_its_altitude() {
        // A plateau 7 m up, seen from 30 m above it, far from the origin.
        let eye = DVec3::new(1003.0, -497.0, -37.0);
        let drawn = ground_drawn_at(7.0, eye);
        assert_eq!(drawn.y, 7.0); // render y is up
        // Under the eye, to within a tile (10 m): it covers what's in view
        // all around.
        let under_eye = to_render(eye);
        assert!(
            (drawn.x - under_eye.x).abs() <= 5.0,
            "{drawn} vs {under_eye}"
        );
        assert!(
            (drawn.z - under_eye.z).abs() <= 5.0,
            "{drawn} vs {under_eye}"
        );
    }

    #[test]
    fn the_ground_moves_in_whole_tiles_so_its_grid_stays_put() {
        // Wherever the camera is, the ground is moved by whole major cells
        // (10 m): its texture repeats every tile, so the grid lines land
        // exactly where they were.
        for eye in [
            DVec3::new(3.0, 4.0, -9.0),
            DVec3::new(-1234.5, 678.9, -20.0),
        ] {
            let drawn = ground_drawn_at(0.0, eye);
            assert_eq!((drawn.x % 10.0, drawn.z % 10.0), (0.0, 0.0), "{drawn}");
        }
    }

    #[test]
    fn proxies_are_created_once_not_every_frame() {
        // Retained mode: the proxy lives across frames; only its position
        // is updated.
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer) = setup(&ball_on_the_ground(), &mut backend);
        for _ in 0..3 {
            renderer.draw(&mut backend, sim.view(), from(DVec3::new(0.0, 0.0, -10.0)));
        }
        assert_eq!(backend.proxies.len(), 2);
    }

    #[test]
    fn the_view_is_sent_in_render_axes() {
        let mut backend = RecordingBackend::new(SCREEN);
        let view = View {
            eye: DVec3::new(10.0, -20.0, -30.0),
            target: DVec3::new(40.0, 12.0, -5.0),
        };
        let (sim, renderer) = setup(&ball_on_the_ground(), &mut backend);
        renderer.draw(&mut backend, sim.view(), view);
        let camera = backend.camera.expect("a camera is set");
        assert_eq!(camera, (to_render(view.eye), to_render(view.target)));
    }

    #[test]
    fn colors_are_opaque_unless_said_otherwise() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3).a, 1.0);
    }
}
