//! Rendering: every decision about *how the world is drawn*, written
//! against a small generic interface (`RenderBackend`) rather than a
//! graphics library, so it can be tested.
//!
//! The layering follows established engines (Godot's `RenderingServer`,
//! Unreal's RHI and scene proxies, Bevy's render world):
//! - **`RenderBackend`**: a handful of generic operations. Upload a
//!   texture, create a render proxy from a mesh, move it, set the camera,
//!   draw text. It knows meshes and textures, not scene objects. It never
//!   grows with content: a plane or a building is a mesh (data), not a new
//!   function.
//! - **`Kiss3dBackend`** (crate `engine_kiss3d`) implements it with one
//!   kiss3d call per method: a "humble object", too simple to hide bugs,
//!   and the only rendering code tests can't run.
//! - **`Renderer`** (here) makes the world's drawing *decisions*: what
//!   mesh each visual becomes, which proxy goes where, f64 -> f32, what
//!   the camera sends. Tests run it against a fake backend that records
//!   calls (the idea behind Unreal's "null RHI").

use glam::{Vec2, Vec3};
use physics::DVec3;

use crate::mesh;
use crate::scene::Scene;
use crate::simulation::WorldView;
use crate::texture::Texture;

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

/// The geometry of a [`Visual`], in metres: the mesh primitives the
/// engine builds (like Godot's `SphereMesh` and `PlaneMesh`).
// TODO(later): `Mesh(asset)` for our own 3D models (glTF from Blender).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Centred on its object's position.
    Sphere { radius: f32 },
    /// Horizontal, through its object's position.
    Plane(PlaneSize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneSize {
    /// A square, `half_size` metres from its centre to each side.
    Square { half_size: f32 },
    /// Endless, like a `HalfSpace` collider: drawn as far as the camera
    /// sees, wherever the camera is.
    Endless,
}

/// What a surface looks like (Godot's `StandardMaterial3D`, Bevy's
/// `StandardMaterial`, a Unity material).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// The surface's colour; with a texture, it tints it (white shows the
    /// texture as it is).
    pub color: Color,
    pub texture: Option<TiledTexture>,
}

impl Material {
    /// A plain colour, no texture.
    pub const fn plain(color: Color) -> Self {
        Self {
            color,
            texture: None,
        }
    }
}

/// A texture covering a surface, repeated every `tile_size` metres (like
/// a material's "tiling" or UV scale in engines).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiledTexture {
    pub texture: TextureId,
    pub tile_size: f64,
}

/// A texture of a scene: its index in `Scene::textures`. Several objects
/// can share one texture, uploaded once (like a Bevy `Handle<Image>`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureId(pub usize);

/// What an object looks like: data, not code. Engines call this a mesh +
/// material (Unity's MeshRenderer, Godot's MeshInstance, Bevy's `Mesh3d`
/// + `MeshMaterial3d` components). The physics never sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub shape: Shape,
    pub material: Material,
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

/// A texture uploaded to the backend (like Godot's texture `RID`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuTexture(pub usize);

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

    /// Uploads a texture, once, for meshes to use. Repeated beyond its
    /// edges, and filtered so it doesn't shimmer far away.
    fn create_texture(&mut self, texture: &Texture) -> GpuTexture;
    /// Retained objects: created once, then moved every frame. A mesh in
    /// `color`, covered with `texture` if any.
    fn create_mesh(&mut self, mesh: &Mesh, color: Color, texture: Option<GpuTexture>) -> ProxyId;
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
    /// Uploads the scene's textures, creates its render proxies, its
    /// background and its light.
    pub(crate) fn new(backend: &mut impl RenderBackend, scene: &Scene) -> Self {
        backend.set_background(scene.background);
        backend.set_sun(to_render(SUN_DIRECTION.normalize()));
        let textures: Vec<GpuTexture> = scene
            .textures
            .iter()
            .map(|texture| backend.create_texture(texture))
            .collect();
        let proxies = scene
            .objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                let Visual { shape, material } = object.visual;
                let texture = material.texture.map(|tiled| {
                    // TODO(step-09): scene validation reports this as an
                    // error.
                    let gpu = textures.get(tiled.texture.0).copied();
                    gpu.unwrap_or_else(|| panic!("object {index}: no texture {}", tiled.texture.0))
                });
                // Without a texture, texture coordinates are unused: any
                // tile size will do.
                let tile = material.texture.map_or(1.0, |tiled| tiled.tile_size);
                let (mesh, placement) = match shape {
                    Shape::Sphere { radius } => {
                        (mesh::sphere(radius, tile as f32), Placement::AtBody)
                    }
                    Shape::Plane(PlaneSize::Square { half_size }) => {
                        (mesh::plane(half_size, tile as f32), Placement::AtBody)
                    }
                    Shape::Plane(PlaneSize::Endless) => (
                        mesh::plane(VIEW_DISTANCE, tile as f32),
                        Placement::UnderCamera { tile },
                    ),
                };
                (
                    backend.create_mesh(&mesh, material.color, texture),
                    placement,
                )
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

    /// A mesh proxy, as the backend was asked to create it.
    #[derive(Debug, PartialEq)]
    pub(crate) struct Created {
        pub(crate) mesh: Mesh,
        pub(crate) color: Color,
        pub(crate) texture: Option<GpuTexture>,
    }

    /// Records every call, so tests can check what was drawn.
    pub(crate) struct RecordingBackend {
        pub(crate) screen: Screen,
        pub(crate) background: Option<Color>,
        pub(crate) sun: Option<Vec3>,
        /// Latest camera: eye and target.
        pub(crate) camera: Option<(Vec3, Vec3)>,
        /// Uploaded textures.
        pub(crate) textures: Vec<Texture>,
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
                textures: Vec::new(),
                proxies: Vec::new(),
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
        fn create_texture(&mut self, texture: &Texture) -> GpuTexture {
            self.textures.push(texture.clone());
            GpuTexture(self.textures.len() - 1)
        }
        fn create_mesh(
            &mut self,
            mesh: &Mesh,
            color: Color,
            texture: Option<GpuTexture>,
        ) -> ProxyId {
            let mesh = mesh.clone();
            self.proxies.push((
                Created {
                    mesh,
                    color,
                    texture,
                },
                None,
            ));
            ProxyId(self.proxies.len() - 1)
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
        fn create_texture(&mut self, _texture: &Texture) -> GpuTexture {
            GpuTexture(0)
        }
        fn create_mesh(&mut self, _: &Mesh, _: Color, _: Option<GpuTexture>) -> ProxyId {
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
        material: Material::plain(WHITE),
    };
    const BACKGROUND: Color = Color::rgb(0.1, 0.2, 0.3);
    /// A 10 m tile of the scene's first texture.
    const TILED: TiledTexture = TiledTexture {
        texture: TextureId(0),
        tile_size: 10.0,
    };

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

    /// Endless flat ground at `elevation` metres, covered with the first
    /// texture.
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
                shape: Shape::Plane(PlaneSize::Endless),
                material: Material {
                    color: WHITE,
                    texture: Some(TILED),
                },
            },
        }
    }

    /// A 1 × 1 texture of `color`.
    fn texture(color: Color) -> Texture {
        Texture::from_fn(1, 1, |_, _| color)
    }

    fn scene(objects: Vec<SceneObject>) -> Scene {
        Scene {
            objects,
            textures: vec![texture(WHITE)],
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
        assert_eq!(backend.background, Some(BACKGROUND));
        let sun = backend.sun.expect("a sun is set");
        assert!(sun.y < 0.0, "sunlight goes down: {sun}");
        assert!((sun.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_sphere_becomes_a_sphere_mesh_in_its_colour() {
        let mut backend = RecordingBackend::new(SCREEN);
        setup(&ball_on_the_ground(), &mut backend);
        let sphere = Created {
            mesh: mesh::sphere(0.4, 1.0),
            color: WHITE,
            texture: None,
        };
        assert_eq!(backend.proxies[1].0, sphere);
    }

    #[test]
    fn a_textured_endless_plane_reaches_the_view_distance_tiled_by_its_texture() {
        let mut backend = RecordingBackend::new(SCREEN);
        setup(&ball_on_the_ground(), &mut backend);
        let created = &backend.proxies[0].0;
        assert_eq!(created.mesh, mesh::plane(VIEW_DISTANCE, 10.0));
        assert_eq!(created.texture, Some(GpuTexture(0)));
    }

    #[test]
    fn a_square_plane_has_its_own_size() {
        let mut square = ground(0.0);
        square.visual.shape = Shape::Plane(PlaneSize::Square { half_size: 25.0 });
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer) = setup(&scene(vec![square]), &mut backend);
        assert_eq!(backend.proxies[0].0.mesh, mesh::plane(25.0, 10.0));
        // A square stays where its body is, wherever the camera goes.
        renderer.draw(
            &mut backend,
            sim.view(),
            from(DVec3::new(123.0, 45.0, -6.0)),
        );
        assert_eq!(backend.proxies[0].1, Some(Vec3::ZERO));
    }

    #[test]
    fn each_texture_is_uploaded_once_however_many_objects_use_it() {
        let mut scene = scene(vec![ground(0.0), ground(5.0)]);
        scene.textures.push(texture(BACKGROUND));
        let mut backend = RecordingBackend::new(SCREEN);
        setup(&scene, &mut backend);
        assert_eq!(backend.textures, scene.textures);
        let used = |i: usize| backend.proxies[i].0.texture;
        assert_eq!(
            (used(0), used(1)),
            (Some(GpuTexture(0)), Some(GpuTexture(0)))
        );
    }

    #[test]
    #[should_panic(expected = "object 1: no texture 3")]
    fn a_missing_texture_is_refused() {
        let mut broken = ground(0.0);
        broken.visual.material.texture = Some(TiledTexture {
            texture: TextureId(3),
            ..TILED
        });
        setup(
            &scene(vec![ground(0.0), broken]),
            &mut RecordingBackend::new(SCREEN),
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

    /// Where the endless ground of a scene with a ground `elevation` metres
    /// up is drawn, seen from `eye`.
    fn ground_drawn_at(elevation: f64, eye: DVec3) -> Vec3 {
        let mut backend = RecordingBackend::new(SCREEN);
        let (sim, renderer) = setup(&scene(vec![ground(elevation)]), &mut backend);
        renderer.draw(&mut backend, sim.view(), from(eye));
        backend.proxies[0].1.expect("the ground is placed")
    }

    #[test]
    fn an_endless_plane_is_drawn_under_the_camera_at_its_altitude() {
        // A plateau 7 m up, seen from 30 m above it, far from the origin.
        let eye = DVec3::new(1003.0, -497.0, -37.0);
        let drawn = ground_drawn_at(7.0, eye);
        assert_eq!(drawn.y, 7.0); // render y is up
        // Under the eye, to within half a tile (10 m): it covers what's in
        // view all around.
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
    fn an_endless_plane_moves_in_whole_tiles_so_its_texture_stays_put() {
        // Moved by whole tiles (10 m): the texture repeats every tile, so
        // it lands exactly where it was.
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
