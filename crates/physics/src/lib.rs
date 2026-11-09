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

/// Standard gravity at the Earth's surface, in m/s², pointing down.
///
/// It's an *acceleration*, the same for every body whatever its mass
/// (Galileo), so a falling ball needs no mass yet.
pub const GRAVITY: DVec2 = DVec2::new(0.0, -9.80665);

/// Something that moves. The smallest possible physics body.
///
/// Plain data, no behaviour: in ECS terms this is a *component*.
/// Behaviour lives in free functions such as [`integrate`] (a *system*).
// TODO(step-04): DVec2 -> DVec3, in a NED frame (x north, y east, z down)
// as in flight-dynamics textbooks; see ROADMAP "Axes".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Position, in metres.
    pub position: DVec2,
    /// Velocity, in metres per second.
    pub velocity: DVec2,
}

/// Everything the simulation knows about.
// TODO(step-11): this hand-written struct becomes an ECS world holding many
// entities (aircraft, buildings). Systems like `integrate` won't change:
// they take components, not the world.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// Simulated time since the start, in seconds. Only `step` advances it.
    pub time: f64,
    /// The ball's motion. `position` is the centre of the ball.
    pub ball: Body,
    /// The ball's collision shape: a circle of this radius, in metres.
    pub ball_radius: f64,
}

impl World {
    pub fn new(ball: Body, ball_radius: f64) -> Self {
        Self {
            time: 0.0,
            ball,
            ball_radius,
        }
    }

    /// Advances the whole simulation by exactly `dt` seconds.
    ///
    /// Callers must always pass the same `dt` (the fixed timestep), see
    /// `FixedTimestep` in the app.
    // TODO(step-06): take the pilot's controls (throttle, pitch) as an
    // argument here; physics reads them but never polls a keyboard.
    pub fn step(&mut self, dt: f64) {
        // TODO(step-06): add aerodynamic forces (lift, drag) and thrust.
        // With forces comes mass: acceleration = total force / mass.
        integrate(&mut self.ball, GRAVITY, dt);
        ground_contact(&mut self.ball, self.ball_radius);
        self.time += dt;
    }
}

/// Advances a body by `dt` seconds under a constant `acceleration` (m/s²),
/// using **semi-implicit (symplectic) Euler** integration:
///
/// ```text
/// v_new = v + a·dt        velocity first...
/// x_new = x + v_new·dt    ...then position, using the *new* velocity
/// ```
///
/// Why this one? Explicit Euler (position from the *old* velocity) is
/// equally simple but slowly *gains* energy: orbits spiral outwards and
/// springs oscillate ever wider, which will matter for landing gear
/// (springs) in step 12. Semi-implicit Euler keeps energy bounded, for the
/// same cost. Its error is proportional to `dt` ("first-order"); that's
/// what the tests check against the exact free-fall formula.
pub fn integrate(body: &mut Body, acceleration: DVec2, dt: f64) {
    body.velocity += acceleration * dt;
    body.position += body.velocity * dt;
}

/// Keeps a ball of `radius` (metres) from going through the ground (y = 0).
///
/// Integration moves in discrete jumps, so a falling ball ends up slightly
/// *inside* the ground on the step it lands. We push it back on top and
/// cancel its downward speed: a perfectly inelastic impact, no bounce.
/// Sideways motion is kept, as there's no friction yet.
// TODO(step-12): replace with landing gear contacts: spring-damper forces
// at each wheel, plus rolling friction and brakes.
pub fn ground_contact(body: &mut Body, radius: f64) {
    if body.position.y < radius {
        body.position.y = radius;
        body.velocity.y = body.velocity.y.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 120.0;
    const RADIUS: f64 = 0.4;
    /// g, as a positive number, in m/s².
    const G: f64 = -GRAVITY.y;

    /// A ball 20 m up, thrown sideways at 5 m/s.
    fn thrown_ball() -> Body {
        Body {
            position: DVec2::new(0.0, 20.0),
            velocity: DVec2::new(5.0, 0.0),
        }
    }

    /// Integrates `body` under gravity alone (no ground) for `seconds`.
    fn fly(mut body: Body, seconds: f64, dt: f64) -> Body {
        let steps = (seconds / dt).round() as u32;
        for _ in 0..steps {
            integrate(&mut body, GRAVITY, dt);
        }
        body
    }

    #[test]
    fn gravity_points_down_with_standard_value() {
        assert_eq!(GRAVITY, DVec2::new(0.0, -9.80665));
    }

    #[test]
    fn without_acceleration_a_body_moves_at_constant_velocity() {
        let mut body = thrown_ball();
        integrate(&mut body, DVec2::ZERO, 0.5);
        assert_eq!(body.position, DVec2::new(2.5, 20.0));
        assert_eq!(body.velocity, DVec2::new(5.0, 0.0));
    }

    #[test]
    fn velocity_is_updated_before_position() {
        // Semi-implicit Euler: the position update already uses the *new*
        // velocity. (Explicit Euler would use the old one and leave the
        // body at y = 0 after this first step.)
        let mut body = Body {
            position: DVec2::ZERO,
            velocity: DVec2::ZERO,
        };
        integrate(&mut body, DVec2::new(0.0, -10.0), 0.5);
        assert_eq!(body.velocity, DVec2::new(0.0, -5.0));
        assert_eq!(body.position, DVec2::new(0.0, -2.5));
    }

    #[test]
    fn free_fall_matches_the_analytic_solution_up_to_the_known_euler_error() {
        // Exact:  y(t) = y0 - g t²/2,   vy(t) = -g t.
        // Semi-implicit Euler with a constant acceleration gives exactly
        //   y_n = y0 - g t²/2 - g t dt/2,
        // i.e. it falls g·t·dt/2 too far (4 cm here). The error is
        // proportional to dt: the method is "first-order accurate".
        let t = 1.0;
        let body = fly(thrown_ball(), t, DT);
        let exact_y = 20.0 - 0.5 * G * t * t;
        let euler_error = 0.5 * G * t * DT;
        assert!((body.position.y - (exact_y - euler_error)).abs() < 1e-9);
        // Velocity under constant acceleration is exact (up to rounding).
        assert!((body.velocity.y - (-G * t)).abs() < 1e-9);
    }

    #[test]
    fn halving_the_timestep_halves_the_error() {
        let exact_y = 20.0 - 0.5 * G;
        let error = |dt: f64| (fly(thrown_ball(), 1.0, dt).position.y - exact_y).abs();
        let ratio = error(DT) / error(DT / 2.0);
        assert!((ratio - 2.0).abs() < 1e-6, "ratio = {ratio}");
    }

    #[test]
    fn gravity_does_not_change_horizontal_motion() {
        let body = fly(thrown_ball(), 1.0, DT);
        assert!((body.position.x - 5.0).abs() < 1e-9);
        assert_eq!(body.velocity.x, 5.0);
    }

    #[test]
    fn ground_contact_puts_a_sunken_ball_back_on_the_ground() {
        let mut body = Body {
            position: DVec2::new(3.0, -0.1),
            velocity: DVec2::new(5.0, -2.0),
        };
        ground_contact(&mut body, RADIUS);
        assert_eq!(body.position, DVec2::new(3.0, RADIUS));
        // Falling stops; no friction yet, so it keeps sliding sideways.
        assert_eq!(body.velocity, DVec2::new(5.0, 0.0));
    }

    #[test]
    fn ground_contact_ignores_a_ball_in_the_air() {
        let mut body = thrown_ball();
        ground_contact(&mut body, RADIUS);
        assert_eq!(body, thrown_ball());
    }

    #[test]
    fn world_starts_at_time_zero() {
        let world = World::new(thrown_ball(), RADIUS);
        assert_eq!(world.time, 0.0);
        assert_eq!(world.ball, thrown_ball());
        assert_eq!(world.ball_radius, RADIUS);
    }

    #[test]
    fn ball_lands_at_the_free_fall_time_and_stays_on_the_ground() {
        // Its centre falls h = 20 m - radius, which takes t = sqrt(2h/g).
        let impact = (2.0 * (20.0 - RADIUS) / G).sqrt();
        let mut world = World::new(thrown_ball(), RADIUS);

        while world.time < impact - 2.0 * DT {
            world.step(DT);
        }
        assert!(world.ball.position.y > RADIUS, "landed too early");

        while world.time < impact + 2.0 * DT {
            world.step(DT);
        }
        assert_eq!(world.ball.position.y, RADIUS, "not landed in time");
        assert_eq!(world.ball.velocity.y, 0.0);

        for _ in 0..600 {
            world.step(DT); // 5 more seconds: it must not sink or bounce
        }
        assert_eq!(world.ball.position.y, RADIUS);
    }

    #[test]
    fn simulation_is_deterministic() {
        // Two worlds fed the same steps must be bit-identical (assert_eq!,
        // not a tolerance). Replays in step 7 rely on this.
        let mut a = World::new(thrown_ball(), RADIUS);
        let mut b = World::new(thrown_ball(), RADIUS);
        for _ in 0..1000 {
            a.step(DT);
            b.step(DT);
        }
        assert_eq!(a, b);
    }
}
