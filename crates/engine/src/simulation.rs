//! The simulation as the game loop sees it: the physics world, the clock
//! that decides how many fixed steps to run, and where to draw each entity
//! between two steps (interpolation).
//!
//! This is the heart of the game loop, kept out of the platform layer so it is
//! tested. Its fields are private to protect one invariant: `previous`
//! always holds the positions *just before* the latest physics step.
//! Nothing outside can step the world without updating it, or forget to
//! reset it on restart (a restart is simply a fresh copy of the loaded
//! `Simulation`).

use std::fmt;

use arena::SecondaryMap;
use physics::{Body, BodyId, DVec3, World, WorldError};

use crate::scene::{EntityId, Scene};
use crate::timestep::FixedTimestep;

/// What a game can see of the world: its data, read-only, and none of the
/// machinery. Cheap to pass by value: 24 bytes, *borrows* of the engine's
/// data, never a copy of it. The physics `World` itself (with `step`)
/// stays inside the engine: as in Godot or Unreal, physics advances on the
/// engine's tick, and gameplay code reads state, it doesn't step the
/// simulation.
#[derive(Clone, Copy)]
pub struct WorldView<'a> {
    /// Simulated time since the start, in seconds.
    pub time: f64,
    world: &'a World,
    entities: &'a SecondaryMap<EntityId, Interpolated>,
}

impl<'a> WorldView<'a> {
    /// The physics state of entity `entity`'s body, if it has one.
    pub fn body(&self, entity: EntityId) -> Option<&'a Body> {
        self.world.body(self.entities.get(entity)?.body)
    }

    /// Where entity `entity` is *drawn* this frame (NED, metres): between
    /// its last two physics states (see `Simulation::advance`). What a
    /// camera should follow, so it moves as smoothly as the picture (like
    /// Unity's interpolated transforms).
    pub fn drawn(&self, entity: EntityId) -> Option<DVec3> {
        Some(self.entities.get(entity)?.drawn)
    }
}

impl fmt::Debug for WorldView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorldView at {} s", self.time)
    }
}

/// How a game acts on the world during a physics step: by pushing
/// entities. The forces it adds act throughout the coming step, then are
/// cleared, so a steady push is added again every step (as with Rapier's
/// `add_force`, reset after each step, or Unity's `AddForce` in
/// `FixedUpdate`). A borrow of the engine's buffer: no allocation.
pub struct Forces<'a> {
    forces: &'a mut SecondaryMap<EntityId, DVec3>,
}

impl Forces<'_> {
    /// Adds `force` (newtons, NED) to what pushes entity `entity` during
    /// this step. An entity without a body is ignored.
    pub fn add(&mut self, entity: EntityId, force: DVec3) {
        if let Some(total) = self.forces.get_mut(entity) {
            *total += force;
        }
    }
}

/// Why a scene can't be loaded: one of its objects makes no sense.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SceneError {
    pub(crate) entity: EntityId,
    pub(crate) error: WorldError,
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object {:?}: {}", self.entity, self.error)
    }
}

/// What the simulation keeps per entity: its body in the physics world,
/// and where to draw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Interpolated {
    body: BodyId,
    /// The body's position just before the latest physics step: what
    /// interpolation blends from.
    ///
    /// Engines keep exactly this, the previous transform of each
    /// interpolated object (Bevy's `PreviousPhysicalTranslation`, Godot's
    /// per-object previous transform, Unity's Rigidbody interpolation),
    /// not a copy of the whole world: velocities and colliders aren't
    /// drawn, so they aren't copied.
    // TODO(step-08): orientations too (quaternions, blended with `slerp`).
    // TODO(step-11): only for *moving* entities; static ones need none.
    previous: DVec3,
    /// Where it's drawn this frame (see `advance`): computed once per
    /// frame, for the renderer and the game alike.
    drawn: DVec3,
}

pub(crate) struct Simulation {
    world: World,
    clock: FixedTimestep,
    /// Per entity: its body and where it's drawn. Overwritten in place
    /// every step and frame: no allocation.
    entities: SecondaryMap<EntityId, Interpolated>,
    /// The forces on each entity during the coming step (see `Forces`),
    /// one per entity from the start, zeroed after each step: no
    /// allocation.
    forces: SecondaryMap<EntityId, DVec3>,
}

impl Simulation {
    /// The physics side of `scene`: each object's body joins a new world
    /// (`Scene::gravity`), simulated in steps of `dt` seconds. Fails on the
    /// first object whose body makes no sense.
    pub(crate) fn load(scene: &Scene, dt: f64) -> Result<Self, SceneError> {
        let mut world = World::new(scene.gravity);
        let mut entities = SecondaryMap::new();
        let mut forces = SecondaryMap::new();
        for (entity, object) in scene.objects() {
            let body = world
                .insert(object.body)
                .map_err(|error| SceneError { entity, error })?;
            let position = object.body.motion.position;
            entities.insert(
                entity,
                Interpolated {
                    body,
                    previous: position,
                    drawn: position,
                },
            );
            forces.insert(entity, DVec3::ZERO);
        }
        Ok(Self {
            world,
            clock: FixedTimestep::new(dt),
            entities,
            forces,
        })
    }

    /// Feeds the real time the last frame took (seconds), runs as many
    /// fixed physics steps as it calls for (often 0, 1 or 2) and returns
    /// how many ran. Then works out where to draw each entity.
    ///
    /// Before each step, `before_step` sees the world as it is and can
    /// push entities for that step (`Forces`): game logic that must run at
    /// the physics rate, not the frame rate (Unity's `FixedUpdate`).
    ///
    /// Drawn positions are `alpha` of the way from each body's position
    /// before the latest step to its current one. Trade-off: what you see
    /// is up to one physics step (8 ms at 120 Hz) behind the simulation.
    /// The alternative, *extrapolating* ahead from the velocity, overshoots
    /// whenever something changes direction (like a ball landing).
    pub(crate) fn advance(
        &mut self,
        frame_time: f64,
        mut before_step: impl FnMut(WorldView<'_>, &mut Forces<'_>),
    ) -> u32 {
        let steps = self.clock.advance(frame_time);
        for _ in 0..steps {
            let view = WorldView {
                time: self.world.time(),
                world: &self.world,
                entities: &self.entities,
            };
            let mut forces = Forces {
                forces: &mut self.forces,
            };
            before_step(view, &mut forces);
            for (entity, force) in self.forces.iter_mut() {
                if let Some(tracked) = self.entities.get(entity) {
                    self.world.add_force(tracked.body, *force);
                }
                *force = DVec3::ZERO;
            }
            for (_, tracked) in self.entities.iter_mut() {
                if let Some(body) = self.world.body(tracked.body) {
                    tracked.previous = body.motion.position;
                }
            }
            self.world.step(self.clock.dt());
        }
        let alpha = self.clock.alpha();
        for (_, tracked) in self.entities.iter_mut() {
            if let Some(body) = self.world.body(tracked.body) {
                tracked.drawn = tracked.previous.lerp(body.motion.position, alpha);
            }
        }
        steps
    }

    /// The world's data, read-only, for games (and the renderer).
    pub(crate) fn view(&self) -> WorldView<'_> {
        WorldView {
            time: self.world.time(),
            world: &self.world,
            entities: &self.entities,
        }
    }

    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    pub(crate) fn clock(&self) -> &FixedTimestep {
        &self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Material, Shape, Visual, WHITE};
    use crate::scene::SceneObject;
    use physics::{Body, BodyType, Collider, Motion};

    /// Exact in binary floating point, so step counts and positions below
    /// can be compared exactly.
    const DT: f64 = 1.0 / 64.0;
    const RADIUS: f64 = 0.5;

    /// A 1 kg ball `north` metres north, sliding north at `speed` (m/s).
    fn sliding_ball(north: f64, speed: f64) -> SceneObject {
        SceneObject {
            body: Body {
                body_type: BodyType::Dynamic { mass: 1.0 },
                motion: Motion {
                    position: DVec3::new(north, 0.0, -RADIUS),
                    velocity: DVec3::new(speed, 0.0, 0.0),
                },
                collider: Collider::Sphere { radius: RADIUS },
                restitution: 0.0,
            },
            visual: Visual {
                shape: Shape::Sphere { radius: 0.5 },
                material: Material::plain(WHITE),
            },
        }
    }

    /// An empty scene without gravity, so no ground is needed: balls just
    /// slide.
    fn weightless() -> Scene {
        let mut scene = Scene::default();
        scene.gravity = DVec3::ZERO;
        scene
    }

    fn load(scene: &Scene) -> Simulation {
        Simulation::load(scene, DT).expect("a valid scene")
    }

    /// One ball sliding at 64 m/s: exactly 1 m per step.
    fn one_ball() -> (Simulation, EntityId) {
        let mut scene = weightless();
        let ball = scene.spawn(sliding_ball(0.0, 64.0));
        (load(&scene), ball)
    }

    /// A game that does nothing to the world between steps.
    fn no_push(_: WorldView<'_>, _: &mut Forces<'_>) {}

    fn body(sim: &Simulation, entity: EntityId) -> Body {
        *sim.view().body(entity).expect("a body")
    }

    fn drawn_north(sim: &Simulation, entity: EntityId) -> f64 {
        sim.view().drawn(entity).expect("drawn").x
    }

    #[test]
    fn a_loaded_simulation_draws_its_current_state() {
        // No blending from some stale earlier state (e.g. after a reload).
        let (sim, ball) = one_ball();
        assert_eq!(drawn_north(&sim, ball), 0.0);
        assert_eq!(body(&sim, ball).motion.velocity.x, 64.0);
    }

    #[test]
    fn loading_refuses_an_impossible_object_naming_it() {
        let mut scene = weightless();
        let mut falling_ground = sliding_ball(0.0, 0.0);
        falling_ground.body.collider = Collider::HalfSpace;
        let entity = scene.spawn(falling_ground);
        let error = Simulation::load(&scene, DT).err().expect("refused");
        assert_eq!(error.entity, entity);
        assert_eq!(error.error, WorldError::MustBeStatic);
        assert_eq!(
            error.to_string(),
            "object EntityId(0v0): a dynamic body's collider can only be static"
        );
    }

    #[test]
    fn advance_runs_as_many_fixed_steps_as_real_time_allows() {
        let (mut sim, ball) = one_ball();
        assert_eq!(sim.advance(2.25 * DT, no_push), 2);
        assert_eq!(sim.world().time(), 2.0 * DT);
        assert_eq!(body(&sim, ball).motion.position.x, 2.0);
        assert_eq!(sim.clock().alpha(), 0.25);
    }

    #[test]
    fn drawn_state_is_between_the_last_two_physics_states() {
        let (mut sim, ball) = one_ball();
        sim.advance(2.25 * DT, no_push); // physics at x = 1 m, then 2 m; 1/4 step left
        assert_eq!(drawn_north(&sim, ball), 1.25);
        assert_eq!(sim.advance(0.5 * DT, no_push), 0); // no step, just further along
        assert_eq!(drawn_north(&sim, ball), 1.75);
    }

    #[test]
    fn every_entity_is_interpolated_with_its_own_previous_position() {
        let mut scene = weightless();
        let first = scene.spawn(sliding_ball(0.0, 64.0));
        let second = scene.spawn(sliding_ball(10.0, -128.0));
        let mut sim = load(&scene);
        sim.advance(2.5 * DT, no_push);
        // First: 1 m -> 2 m; second: 8 m -> 6 m; drawn halfway.
        assert_eq!(drawn_north(&sim, first), 1.5);
        assert_eq!(drawn_north(&sim, second), 7.0);
    }

    #[test]
    fn the_game_acts_before_each_step_on_the_world_as_it_is() {
        // 2.5 steps of time: two steps, so two calls, each seeing the ball
        // where the previous step left it (0 m, then 1 m north).
        let (mut sim, ball) = one_ball();
        let mut seen = Vec::new();
        let steps = sim.advance(2.5 * DT, |world, _| {
            seen.push(world.body(ball).expect("a body").motion.position.x);
        });
        assert_eq!(steps, 2);
        assert_eq!(seen, [0.0, 1.0]);
    }

    #[test]
    fn forces_push_during_their_step_only() {
        // 64 N on the 1 kg ball for one 1/64 s step: +1 m/s north. The
        // next step, nobody pushes: it keeps that speed.
        let (mut sim, ball) = one_ball();
        let mut first = true;
        sim.advance(2.0 * DT, |_, forces| {
            if first {
                forces.add(ball, DVec3::new(64.0, 0.0, 0.0));
                first = false;
            }
        });
        assert_eq!(body(&sim, ball).motion.velocity.x, 64.0 + 1.0);
        assert_eq!(sim.forces.get(ball), Some(&DVec3::ZERO), "cleared");
    }

    #[test]
    fn forces_on_one_entity_add_up() {
        let (mut sim, ball) = one_ball();
        sim.advance(DT, |_, forces| {
            forces.add(ball, DVec3::new(32.0, 0.0, 0.0));
            forces.add(ball, DVec3::new(32.0, 0.0, 0.0));
        });
        assert_eq!(body(&sim, ball).motion.velocity.x, 64.0 + 1.0);
    }

    #[test]
    fn an_entity_of_another_scene_has_nothing_here() {
        // `stranger` is an entity of a scene with two objects; this one
        // has only one.
        let mut other = weightless();
        other.spawn(sliding_ball(0.0, 0.0));
        let stranger = other.spawn(sliding_ball(0.0, 0.0));
        let (mut sim, ball) = one_ball();
        assert_eq!(sim.view().body(stranger), None);
        assert_eq!(sim.view().drawn(stranger), None);
        sim.advance(DT, |_, forces| forces.add(stranger, DVec3::X)); // ignored
        assert_eq!(body(&sim, ball).motion.velocity.x, 64.0);
    }

    #[test]
    fn the_view_borrows_the_engines_data_and_copies_none() {
        // 24 bytes whatever the number of entities: the time and two
        // references (checked at compile time)...
        const { assert!(std::mem::size_of::<WorldView<'_>>() == 24) }
        // ...to the engine's own data, not copies of it.
        let (sim, _) = one_ball();
        let view = sim.view();
        assert!(std::ptr::eq(view.world, &sim.world));
        assert!(std::ptr::eq(view.entities, &sim.entities));
        assert_eq!(view.time, sim.world().time());
        assert_eq!(format!("{view:?}"), "WorldView at 0 s");
    }
}
