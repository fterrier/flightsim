//! The simulation as the game loop sees it: the physics world, the clock
//! that decides how many fixed steps to run, and what to draw between two
//! steps (interpolation).
//!
//! This is the heart of the game loop, kept out of `main.rs` so it is
//! tested. Its fields are private to protect one invariant: `previous` is
//! always the drawn state *just before* the latest physics step. Nothing
//! outside can step the world without updating it, or forget to reset it
//! on restart (a restart is simply a new `Simulation`).

use crate::timestep::FixedTimestep;
use physics::{DVec2, World};

/// The part of a physics state that rendering needs: only what is drawn.
///
/// Before each physics step, the state about to be replaced is captured in
/// one of these, so the renderer can draw *between* it and the current
/// state. Engines do the same, keeping the previous transform of each
/// interpolated object only (Bevy's `PreviousPhysicalTranslation`, Godot's
/// per-object previous transform, Unity's Rigidbody interpolation), not a
/// copy of the whole world: velocities, masses or sim time aren't drawn,
/// so they aren't copied. It's 16 bytes, on the stack, never a heap
/// allocation.
// TODO(step-04): positions become `DVec3`.
// TODO(step-08): add orientations (quaternions), blended with `slerp`.
// TODO(step-11): one entry per *moving* entity, in a contiguous array;
// static objects (buildings) need none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderState {
    /// Ball centre, in metres.
    pub ball: DVec2,
}

impl RenderState {
    pub fn capture(world: &World) -> Self {
        Self {
            ball: world.ball.position,
        }
    }

    /// The state to draw, `alpha` of the way (0 to 1) from `self` (the
    /// previous state) to `current`.
    pub fn interpolate(&self, current: &Self, alpha: f64) -> Self {
        Self {
            ball: self.ball.lerp(current.ball, alpha),
        }
    }
}

pub struct Simulation {
    world: World,
    clock: FixedTimestep,
    /// Drawn state just before the latest physics step.
    previous: RenderState,
}

impl Simulation {
    /// Starts simulating `world` with physics steps of `dt` seconds.
    pub fn new(world: World, dt: f64) -> Self {
        let previous = RenderState::capture(&world);
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
            self.previous = RenderState::capture(&self.world);
            self.world.step(self.clock.dt());
        }
        steps
    }

    /// What to draw now: `alpha` of the way from the state before the
    /// latest step to the current one.
    ///
    /// Trade-off: what you see is up to one physics step (8 ms at 120 Hz)
    /// behind the simulation. The alternative, *extrapolating* ahead from
    /// the velocity, overshoots whenever something changes direction (like
    /// the ball landing).
    pub fn drawn(&self) -> RenderState {
        self.previous
            .interpolate(&RenderState::capture(&self.world), self.clock.alpha())
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn clock(&self) -> &FixedTimestep {
        &self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{Body, Circle};

    /// Exact in binary floating point, so step counts and positions below
    /// can be compared exactly.
    const DT: f64 = 1.0 / 64.0;
    const RADIUS: f64 = 0.5;

    /// A ball resting on the ground, sliding right at 64 m/s: exactly 1 m
    /// per step, and the ground stops gravity from moving it vertically.
    fn sliding_ball() -> Simulation {
        let ball = Body {
            position: DVec2::new(0.0, RADIUS),
            velocity: DVec2::new(64.0, 0.0),
        };
        Simulation::new(World::new(ball, Circle { radius: RADIUS }), DT)
    }

    #[test]
    fn render_state_captures_only_what_is_drawn() {
        // The doc comment promises 16 bytes; checked at compile time.
        const { assert!(std::mem::size_of::<RenderState>() == 16) }
        let sim = sliding_ball();
        assert_eq!(
            RenderState::capture(sim.world()),
            RenderState {
                ball: DVec2::new(0.0, RADIUS)
            }
        );
    }

    #[test]
    fn interpolation_blends_two_render_states() {
        let previous = RenderState {
            ball: DVec2::new(0.0, 10.0),
        };
        let current = RenderState {
            ball: DVec2::new(4.0, 6.0),
        };
        assert_eq!(previous.interpolate(&current, 0.0), previous);
        assert_eq!(
            previous.interpolate(&current, 0.25).ball,
            DVec2::new(1.0, 9.0)
        );
        assert_eq!(previous.interpolate(&current, 1.0), current);
    }

    #[test]
    fn a_new_simulation_draws_its_current_state() {
        // No blending from some stale earlier state (e.g. after a restart).
        let sim = sliding_ball();
        assert_eq!(sim.drawn().ball, DVec2::new(0.0, RADIUS));
    }

    #[test]
    fn advance_runs_as_many_fixed_steps_as_real_time_allows() {
        let mut sim = sliding_ball();
        assert_eq!(sim.advance(2.25 * DT), 2);
        assert_eq!(sim.world().time, 2.0 * DT);
        assert_eq!(sim.world().ball.position.x, 2.0);
        assert_eq!(sim.clock().alpha(), 0.25);
    }

    #[test]
    fn drawn_state_is_between_the_last_two_physics_states() {
        let mut sim = sliding_ball();
        sim.advance(2.25 * DT); // physics at x = 1 m, then 2 m; 1/4 step left
        assert_eq!(sim.drawn().ball.x, 1.25);
        assert_eq!(sim.advance(0.5 * DT), 0); // no step, just further along
        assert_eq!(sim.drawn().ball.x, 1.75);
    }
}
