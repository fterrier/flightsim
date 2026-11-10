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
use physics::{DVec2, World};

pub struct Simulation {
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
    previous: Vec<DVec2>,
}

impl Simulation {
    /// Starts simulating `world` with physics steps of `dt` seconds.
    pub fn new(world: World, dt: f64) -> Self {
        let previous = positions(&world).collect();
        Self {
            world,
            clock: FixedTimestep::new(dt),
            previous,
        }
    }

    /// Feeds the real time the last frame took (seconds), runs as many
    /// fixed physics steps as it calls for (often 0, 1 or 2) and returns
    /// how many ran.
    // TODO(step-06): take the pilot's controls and pass them to each step.
    pub fn advance(&mut self, frame_time: f64) -> u32 {
        let steps = self.clock.advance(frame_time);
        for _ in 0..steps {
            for (previous, current) in self.previous.iter_mut().zip(positions(&self.world)) {
                *previous = current;
            }
            self.world.step(self.clock.dt());
        }
        steps
    }

    /// Where to draw each body now, in the order of `world().bodies`:
    /// `alpha` of the way from its position before the latest step to its
    /// current one.
    ///
    /// Trade-off: what you see is up to one physics step (8 ms at 120 Hz)
    /// behind the simulation. The alternative, *extrapolating* ahead from
    /// the velocity, overshoots whenever something changes direction (like
    /// a ball landing).
    pub fn drawn_positions(&self) -> impl Iterator<Item = DVec2> + '_ {
        let alpha = self.clock.alpha();
        self.previous
            .iter()
            .zip(positions(&self.world))
            .map(move |(previous, current)| previous.lerp(current, alpha))
    }

    /// Where to draw body number `index` now (see `drawn_positions`).
    pub fn drawn_position(&self, index: usize) -> DVec2 {
        let current = self.world.bodies[index].motion.position;
        self.previous[index].lerp(current, self.clock.alpha())
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn clock(&self) -> &FixedTimestep {
        &self.clock
    }
}

fn positions(world: &World) -> impl Iterator<Item = DVec2> + '_ {
    world.bodies.iter().map(|body| body.motion.position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{Body, Circle, Motion};

    /// Exact in binary floating point, so step counts and positions below
    /// can be compared exactly.
    const DT: f64 = 1.0 / 64.0;
    const RADIUS: f64 = 0.5;

    /// A ball resting on the ground at `x`, sliding right at `speed`
    /// (m/s); the ground stops gravity from moving it vertically.
    fn sliding_ball(x: f64, speed: f64) -> Body {
        Body {
            motion: Motion {
                position: DVec2::new(x, RADIUS),
                velocity: DVec2::new(speed, 0.0),
            },
            collider: Circle { radius: RADIUS },
        }
    }

    /// One ball sliding at 64 m/s: exactly 1 m per step.
    fn one_ball() -> Simulation {
        Simulation::new(World::new(vec![sliding_ball(0.0, 64.0)]), DT)
    }

    fn drawn_x(sim: &Simulation) -> Vec<f64> {
        sim.drawn_positions().map(|p| p.x).collect()
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
        assert_eq!(sim.advance(2.25 * DT), 2);
        assert_eq!(sim.world().time(), 2.0 * DT);
        assert_eq!(sim.world().bodies[0].motion.position.x, 2.0);
        assert_eq!(sim.clock().alpha(), 0.25);
    }

    #[test]
    fn drawn_state_is_between_the_last_two_physics_states() {
        let mut sim = one_ball();
        sim.advance(2.25 * DT); // physics at x = 1 m, then 2 m; 1/4 step left
        assert_eq!(drawn_x(&sim), [1.25]);
        assert_eq!(sim.advance(0.5 * DT), 0); // no step, just further along
        assert_eq!(drawn_x(&sim), [1.75]);
    }

    #[test]
    fn every_body_is_interpolated_with_its_own_previous_position() {
        let world = World::new(vec![sliding_ball(0.0, 64.0), sliding_ball(10.0, -128.0)]);
        let mut sim = Simulation::new(world, DT);
        sim.advance(2.5 * DT);
        // Ball 0: 1 m -> 2 m; ball 1: 8 m -> 6 m; drawn halfway.
        assert_eq!(drawn_x(&sim), [1.5, 7.0]);
        assert_eq!(sim.drawn_position(1).x, 7.0);
    }
}
