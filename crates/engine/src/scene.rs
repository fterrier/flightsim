//! The scene format: everything that exists in the world, as objects, each
//! described *whole* (physics and looks together), the textures they use,
//! and the scene-wide settings (background, gravity). Like a Godot scene, a
//! Unity scene or prefab, an Unreal level. The engine defines the format;
//! games fill it in.
//!
//! A game *spawns* objects into the scene and adds textures to it, and is
//! handed an ID for each, as in Bevy (`commands.spawn` returns an `Entity`,
//! `assets.add` a `Handle`) or Unity (`Instantiate` returns the object). It
//! keeps the IDs it needs, such as which ball to push.
//!
//! As in engines, the ground is not special: it's an object like a ball,
//! with a body that never moves (a *static* body, like Rapier's fixed
//! bodies or Godot's `StaticBody3D`). Only ambient settings that exist
//! nowhere in particular (background, gravity) are scene fields, like
//! Godot's `WorldEnvironment` or Unity's render and physics settings.
//!
//! Loading splits each object: its body goes to the simulation, its visual
//! to the renderer, both under the object's `EntityId`.

use arena::Arena;
use physics::{Body, DVec3, STANDARD_GRAVITY};

use crate::render::{Color, Visual};
use crate::texture::Texture;

arena::id_type! {
    /// Identifies an object of the world: an *entity*. Handed out by
    /// `Scene::spawn`; the game uses it to read the object's state
    /// (`WorldView`) and act on it (`Forces`). Opaque and generational (see
    /// the `arena` crate), like Bevy's `Entity`.
    pub struct EntityId;
}

arena::id_type! {
    /// Identifies a texture of a scene. Handed out by `Scene::add_texture`;
    /// materials refer to it, so a texture used by many objects is stored
    /// and uploaded once (like Bevy's `Handle<Image>`).
    pub struct TextureId;
}

/// One object of a scene: everything about it, in one place.
// TODO(step-11): becomes an ECS "bundle": the group of components an
// entity is spawned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    /// How it moves and collides (or doesn't: static bodies): for the
    /// simulation.
    pub body: Body,
    /// What it looks like: for the renderer.
    pub visual: Visual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Everything that exists in the world, moving or static (the ground),
    /// by entity.
    objects: Arena<EntityId, SceneObject>,
    /// The images objects' materials use.
    textures: Arena<TextureId, Texture>,
    /// Sky colour, in linear light (see `render::Color`).
    pub background: Color,
    /// The world's gravity, in m/s².
    pub gravity: DVec3,
}

impl Default for Scene {
    /// An empty scene: black sky, Earth's standard gravity.
    fn default() -> Self {
        Self {
            objects: Arena::new(),
            textures: Arena::new(),
            background: Color::rgb(0.0, 0.0, 0.0),
            gravity: STANDARD_GRAVITY,
        }
    }
}

impl Scene {
    /// Adds `object` to the scene and returns its new entity ID.
    pub fn spawn(&mut self, object: SceneObject) -> EntityId {
        self.objects.insert(object)
    }

    /// Adds `texture` to the scene and returns its ID, for materials.
    pub fn add_texture(&mut self, texture: Texture) -> TextureId {
        self.textures.insert(texture)
    }

    /// Every object with its entity ID, in the order they were spawned.
    pub(crate) fn objects(&self) -> impl Iterator<Item = (EntityId, &SceneObject)> {
        self.objects.iter()
    }

    /// Every texture with its ID.
    pub(crate) fn textures(&self) -> impl Iterator<Item = (TextureId, &Texture)> {
        self.textures.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Material, Shape, WHITE};
    use physics::{BodyType, Collider, Motion};

    fn ball() -> SceneObject {
        SceneObject {
            body: Body {
                body_type: BodyType::Dynamic { mass: 1.0 },
                motion: Motion {
                    position: DVec3::new(1.0, 2.0, -3.0),
                    velocity: DVec3::ZERO,
                },
                collider: Collider::Sphere { radius: 1.0 },
            },
            visual: Visual {
                shape: Shape::Sphere { radius: 1.0 },
                material: Material::plain(WHITE),
            },
        }
    }

    #[test]
    fn spawned_objects_are_listed_with_their_ids_in_spawn_order() {
        let mut scene = Scene::default();
        let first = scene.spawn(ball());
        let mut other = ball();
        other.body.motion.position.x = 9.0;
        let second = scene.spawn(other);
        assert_ne!(first, second);
        let listed: Vec<(EntityId, SceneObject)> =
            scene.objects().map(|(id, o)| (id, *o)).collect();
        assert_eq!(listed, [(first, ball()), (second, other)]);
    }

    #[test]
    fn added_textures_are_listed_with_their_ids() {
        let mut scene = Scene::default();
        let black = Texture::from_fn(1, 1, |_, _| Color::rgb(0.0, 0.0, 0.0));
        let id = scene.add_texture(black.clone());
        let listed: Vec<(TextureId, &Texture)> = scene.textures().collect();
        assert_eq!(listed, [(id, &black)]);
    }

    #[test]
    fn a_new_scene_is_empty_under_standard_gravity() {
        let scene = Scene::default();
        assert_eq!(scene.objects().count(), 0);
        assert_eq!(scene.gravity, STANDARD_GRAVITY);
    }
}
