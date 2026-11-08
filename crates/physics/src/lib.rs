//! Flight physics: pure, deterministic simulation code.
//!
//! Rules for this crate (see ROADMAP.md):
//! - No I/O, no clock, no graphics. Time only enters as a `dt` argument.
//! - Same inputs => same outputs, bit for bit. This is what makes tests
//!   and (later) replays possible.
//! - SI units everywhere: metres, seconds, kilograms, radians.
//!
//! Coordinates (2D steps): x points right, y points up, origin on the ground.

/// Re-exported so the app uses exactly the same vector type as the physics.
pub use glam::DVec2;

/// Something that moves. The smallest possible physics body.
///
/// Plain data, no behaviour: in ECS terms this is a *component*.
/// Behaviour lives in free functions such as [`move_body`] (a *system*).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Position, in metres.
    pub position: DVec2,
    /// Velocity, in metres per second.
    pub velocity: DVec2,
}

/// Everything the simulation knows about.
// TODO(step-08): this hand-written struct becomes an ECS world holding many
// entities (aircraft, buildings). Systems like `move_body` won't change:
// they take components, not the world.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// Simulated time since the start, in seconds. Only `step` advances it.
    pub time: f64,
    pub dot: Body,
}

impl World {
    pub fn new(dot: Body) -> Self {
        Self { time: 0.0, dot }
    }

    /// Advances the whole simulation by exactly `dt` seconds.
    ///
    /// Callers must always pass the same `dt` (the fixed timestep), see
    /// `FixedTimestep` in the app.
    // TODO(step-03): take the pilot's controls (throttle, pitch) as an
    // argument here; physics reads them but never polls a keyboard.
    pub fn step(&mut self, dt: f64) {
        move_body(&mut self.dot, dt);
        self.time += dt;
    }
}

/// Moves a body along its velocity for `dt` seconds.
// TODO(step-02): forces arrive here. Gravity gives an acceleration, which
// changes the velocity first, then the new velocity moves the position
// (semi-implicit Euler integration).
pub fn move_body(body: &mut Body, dt: f64) {
    body.position += body.velocity * dt;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dot 2 m above the ground, moving right at 10 m/s.
    fn moving_dot() -> Body {
        Body {
            position: DVec2::new(0.0, 2.0),
            velocity: DVec2::new(10.0, 0.0),
        }
    }

    #[test]
    fn body_moves_by_velocity_times_dt() {
        let mut body = moving_dot();
        move_body(&mut body, 0.5);
        assert_eq!(body.position, DVec2::new(5.0, 2.0));
        // No forces yet, so velocity never changes.
        assert_eq!(body.velocity, DVec2::new(10.0, 0.0));
    }

    #[test]
    fn world_starts_at_time_zero() {
        let world = World::new(moving_dot());
        assert_eq!(world.time, 0.0);
        assert_eq!(world.dot, moving_dot());
    }

    #[test]
    fn many_small_steps_match_constant_velocity_formula() {
        // Analytic solution: x(t) = x0 + v * t.
        // 1/120 is not exact in binary floating point, so each step adds a
        // tiny rounding error; 1e-9 m over 360 steps is plenty of margin.
        let mut world = World::new(moving_dot());
        for _ in 0..360 {
            world.step(1.0 / 120.0);
        }
        assert!((world.time - 3.0).abs() < 1e-9);
        assert!((world.dot.position - DVec2::new(30.0, 2.0)).length() < 1e-9);
    }

    #[test]
    fn simulation_is_deterministic() {
        // Two worlds fed the same steps must be bit-identical (assert_eq!,
        // not a tolerance). Replays in step 7 rely on this.
        let mut a = World::new(moving_dot());
        let mut b = World::new(moving_dot());
        for _ in 0..1000 {
            a.step(1.0 / 120.0);
            b.step(1.0 / 120.0);
        }
        assert_eq!(a, b);
    }
}
