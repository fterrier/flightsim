//! The simulation as the game loop sees it: the physics world, the clock
//! that decides how many fixed steps to run, and where to draw each body
//! between two steps (interpolation).
//!
//! This is the heart of the game loop, kept out of the platform layer so it is
//! tested. Its fields are private to protect one invariant: `previous`
//! always holds the positions *just before* the latest physics step.
//! Nothing outside can step the world without updating it, or forget to
//! reset it on restart (a restart is simply a new `Simulation`).

use crate::timestep::FixedTimestep;
use physics::{Body, DVec3, World};

/// What a game can see of the world: its data, read-only, and none of the
/// machinery. Cheap to pass by value: 40 bytes, *borrows* of the engine's
/// arrays (pointer + length each), never a copy of them. The physics
/// `World` itself (with `step`) stays inside the engine: as in Godot or
/// Unreal, physics advances on the engine's tick, and gameplay code reads
/// state, it doesn't step the simulation.
#[derive(Clone, Copy, Debug)]
pub struct WorldView<'a> {
    /// Simulated time since the start, in seconds.
    pub time: f64,
    /// All simulated objects, in scene order: their physics state. Shared
    /// slices: they can be read, not changed.
    pub bodies: &'a [Body],
    /// Where each body is *drawn* this frame (NED, metres), in the same
    /// order: between its last two physics states (see `Simulation`).
    /// What a camera should follow, so it moves as smoothly as the
    /// picture (like Unity's interpolated transforms).
    pub drawn: &'a [DVec3],
}

/// How a game acts on the world during a physics step: by pushing bodies.
/// The forces it adds act throughout the coming step, then are cleared,
/// so a steady push is added again every step (as with Rapier's
/// `add_force`, reset after each step, or Unity's `AddForce` in
/// `FixedUpdate`). A borrow of the engine's buffer: no allocation.
pub struct Forces<'a> {
    forces: &'a mut [DVec3],
}

impl<'a> Forces<'a> {
    /// Forces on bodies `0..forces.len()`, accumulated into `forces`. The
    /// engine builds one per step over its own buffer; a game's tests can
    /// build one too, to check what the game pushes.
    pub fn new(forces: &'a mut [DVec3]) -> Self {
        Self { forces }
    }

    /// Adds `force` (newtons, NED) to what pushes body number `body` (as
    /// in `WorldView::bodies`) during this step.
    pub fn add(&mut self, body: usize, force: DVec3) {
        self.forces[body] += force;
    }
}

pub(crate) struct Simulation {
    world: World,
    clock: FixedTimestep,
    /// Each body's position just before the latest physics step, in the
    /// same order as `world.bodies`: what interpolation blends from.
    ///
    /// Engines keep exactly this, the previous transform of each
    /// interpolated object (Bevy's `PreviousPhysicalTranslation`, Godot's
    /// per-object previous transform, Unity's Rigidbody interpolation),
    /// not a copy of the whole world: velocities and colliders aren't
    /// drawn, so they aren't copied. One contiguous array, overwritten in
    /// place every step: no allocation.
    // TODO(step-08): orientations too (quaternions, blended with `slerp`).
    // TODO(step-11): only for *moving* entities; static ones need none.
    previous: Vec<DVec3>,
    /// Where each body is drawn this frame (see `advance`): computed once
    /// per frame, for the renderer and the game alike. Overwritten in
    /// place: no allocation.
    drawn: Vec<DVec3>,
    /// The forces on each body during the coming step (see `Forces`),
    /// zeroed after each step. Reused: no allocation.
    forces: Vec<DVec3>,
}

impl Simulation {
    /// Starts simulating `world` with physics steps of `dt` seconds.
    pub(crate) fn new(world: World, dt: f64) -> Self {
        let previous: Vec<DVec3> = positions(&world).collect();
        Self {
            clock: FixedTimestep::new(dt),
            drawn: previous.clone(),
            forces: vec![DVec3::ZERO; previous.len()],
            previous,
            world,
        }
    }

    /// Feeds the real time the last frame took (seconds), runs as many
    /// fixed physics steps as it calls for (often 0, 1 or 2) and returns
    /// how many ran. Then works out where to draw each body.
    ///
    /// Before each step, `before_step` sees the bodies as they are and can
    /// push them for that step (`Forces`): game logic that must run at the
    /// physics rate, not the frame rate (Unity's `FixedUpdate`).
    ///
    /// Drawn positions are `alpha` of the way from each body's position
    /// before the latest step to its current one. Trade-off: what you see
    /// is up to one physics step (8 ms at 120 Hz) behind the simulation.
    /// The alternative, *extrapolating* ahead from the velocity, overshoots
    /// whenever something changes direction (like a ball landing).
    pub(crate) fn advance(
        &mut self,
        frame_time: f64,
        mut before_step: impl FnMut(&[Body], &mut Forces<'_>),
    ) -> u32 {
        let steps = self.clock.advance(frame_time);
        for _ in 0..steps {
            let mut forces = Forces::new(&mut self.forces);
            before_step(&self.world.bodies, &mut forces);
            for (previous, current) in self.previous.iter_mut().zip(positions(&self.world)) {
                *previous = current;
            }
            self.world.step(self.clock.dt(), &self.forces);
            self.forces.fill(DVec3::ZERO);
        }
        let alpha = self.clock.alpha();
        let blended = self.previous.iter().zip(positions(&self.world));
        for (drawn, (previous, current)) in self.drawn.iter_mut().zip(blended) {
            *drawn = previous.lerp(current, alpha);
        }
        steps
    }

    /// The world's data, read-only, for games (and the renderer).
    pub(crate) fn view(&self) -> WorldView<'_> {
        WorldView {
            time: self.world.time(),
            bodies: &self.world.bodies,
            drawn: &self.drawn,
        }
    }

    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    pub(crate) fn clock(&self) -> &FixedTimestep {
        &self.clock
    }
}

fn positions(world: &World) -> impl Iterator<Item = DVec3> + '_ {
    world.bodies.iter().map(|body| body.motion.position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{Body, BodyType, Collider, Motion};

    /// No gravity, and so no ground needed: balls just slide.
    const NO_GRAVITY: DVec3 = DVec3::ZERO;

    /// Exact in binary floating point, so step counts and positions below
    /// can be compared exactly.
    const DT: f64 = 1.0 / 64.0;
    const RADIUS: f64 = 0.5;

    /// A ball `north` metres north, sliding north at `speed` (m/s).
    fn sliding_ball(north: f64, speed: f64) -> Body {
        Body {
            body_type: BodyType::Dynamic { mass: 1.0 },
            motion: Motion {
                position: DVec3::new(north, 0.0, -RADIUS),
                velocity: DVec3::new(speed, 0.0, 0.0),
            },
            collider: Collider::Sphere { radius: RADIUS },
        }
    }

    /// One ball sliding at 64 m/s: exactly 1 m per step.
    fn one_ball() -> Simulation {
        let world = World::new(vec![sliding_ball(0.0, 64.0)], NO_GRAVITY);
        Simulation::new(world.expect("valid bodies"), DT)
    }

    /// A game that does nothing to the world between steps.
    fn no_push(_: &[Body], _: &mut Forces<'_>) {}

    fn drawn_x(sim: &Simulation) -> Vec<f64> {
        sim.view().drawn.iter().map(|p| p.x).collect()
    }

    #[test]
    fn a_new_simulation_draws_its_current_state() {
        // No blending from some stale earlier state (e.g. after a restart).
        let sim = one_ball();
        assert_eq!(drawn_x(&sim), [0.0]);
    }

    #[test]
    fn advance_runs_as_many_fixed_steps_as_real_time_allows() {
        let mut sim = one_ball();
        assert_eq!(sim.advance(2.25 * DT, no_push), 2);
        assert_eq!(sim.world().time(), 2.0 * DT);
        assert_eq!(sim.world().bodies[0].motion.position.x, 2.0);
        assert_eq!(sim.clock().alpha(), 0.25);
    }

    #[test]
    fn drawn_state_is_between_the_last_two_physics_states() {
        let mut sim = one_ball();
        sim.advance(2.25 * DT, no_push); // physics at x = 1 m, then 2 m; 1/4 step left
        assert_eq!(drawn_x(&sim), [1.25]);
        assert_eq!(sim.advance(0.5 * DT, no_push), 0); // no step, just further along
        assert_eq!(drawn_x(&sim), [1.75]);
    }

    #[test]
    fn the_game_acts_before_each_step_on_the_bodies_as_they_are() {
        // 2.5 steps of time: two steps, so two calls, each seeing the ball
        // where the previous step left it (0 m, then 1 m north).
        let mut sim = one_ball();
        let mut seen = Vec::new();
        let steps = sim.advance(2.5 * DT, |bodies, _| {
            seen.push(bodies[0].motion.position.x);
        });
        assert_eq!(steps, 2);
        assert_eq!(seen, [0.0, 1.0]);
    }

    #[test]
    fn forces_push_during_their_step_only() {
        // 64 N on the 1 kg ball for one 1/64 s step: +1 m/s north. The
        // next step, nobody pushes: it keeps that speed.
        let mut sim = one_ball();
        let mut first = true;
        sim.advance(2.0 * DT, |_, forces| {
            if first {
                forces.add(0, DVec3::new(64.0, 0.0, 0.0));
                first = false;
            }
        });
        assert_eq!(sim.world().bodies[0].motion.velocity.x, 64.0 + 1.0);
        assert!(sim.forces.iter().all(|f| *f == DVec3::ZERO), "cleared");
    }

    #[test]
    fn forces_on_one_body_add_up() {
        let mut sim = one_ball();
        sim.advance(DT, |_, forces| {
            forces.add(0, DVec3::new(32.0, 0.0, 0.0));
            forces.add(0, DVec3::new(32.0, 0.0, 0.0));
        });
        assert_eq!(sim.world().bodies[0].motion.velocity.x, 64.0 + 1.0);
    }

    #[test]
    fn the_view_shows_the_world_data_read_only() {
        let mut sim = one_ball();
        sim.advance(2.0 * DT, no_push);
        let view = sim.view();
        assert_eq!(view.time, sim.world().time());
        assert_eq!(view.bodies, sim.world().bodies.as_slice());
    }

    #[test]
    fn the_view_copies_no_body_data() {
        // 40 bytes whatever the number of bodies: the time, plus a pointer
        // and a length into each of two arrays (checked at compile time).
        const { assert!(std::mem::size_of::<WorldView<'_>>() == 40) }
        // And those pointers are the engine's memory itself, not copies.
        let sim = one_ball();
        assert!(std::ptr::eq(
            sim.view().bodies,
            sim.world().bodies.as_slice()
        ));
        assert!(std::ptr::eq(sim.view().drawn, sim.drawn.as_slice()));
    }

    #[test]
    fn every_body_is_interpolated_with_its_own_previous_position() {
        let world = World::new(
            vec![sliding_ball(0.0, 64.0), sliding_ball(10.0, -128.0)],
            NO_GRAVITY,
        );
        let mut sim = Simulation::new(world.expect("valid bodies"), DT);
        sim.advance(2.5 * DT, no_push);
        // Ball 0: 1 m -> 2 m; ball 1: 8 m -> 6 m; drawn halfway.
        assert_eq!(drawn_x(&sim), [1.5, 7.0]);
    }
}
