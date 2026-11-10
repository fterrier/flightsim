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

/// How something moves: where it is and how fast it goes.
///
/// Plain data, no behaviour: in ECS terms this is a *component*.
/// Behaviour lives in free functions such as [`integrate`] (a *system*).
// TODO(step-04): DVec2 -> DVec3, in a NED frame (x north, y east, z down)
// as in flight-dynamics textbooks; see ROADMAP "Axes".
// TODO(step-06): mass arrives with forces (acceleration = force / mass).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Position, in metres.
    pub position: DVec2,
    /// Velocity, in metres per second.
    pub velocity: DVec2,
}

/// A collision shape: a circle of `radius` metres, centred on its body's
/// position.
// TODO(step-04): becomes a sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    /// In metres.
    pub radius: f64,
}

/// A simulated object: how it moves, and what it bumps into.
///
/// Every physics engine attaches colliders to a body (Box2D creates shapes
/// on a body, Rapier inserts colliders "with parent", Unity puts colliders
/// on the Rigidbody's GameObject): the collider moves with its body. Kept
/// as separate components so systems can take only what they need.
// TODO(step-12): several colliders per body (fuselage, wings, wheels), as
// engines' "compound colliders".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub motion: Motion,
    pub collider: Circle,
}

/// Everything the simulation knows about.
// TODO(step-11): bodies become ECS entities; the `Vec` index becomes an
// entity ID (a generational handle, safe once objects can be removed).
// Systems like `integrate` won't change: they take components.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// Simulated time since the start, in seconds. Private, so only `step`
    /// can advance it: read it with [`World::time`].
    // TODO(step-10): replays identify moments by step number; count steps
    // in a `u64` (exact) and derive time as steps × dt, instead of summing
    // a float that slowly accumulates rounding error.
    time: f64,
    /// All simulated objects, stored contiguously. Their index identifies
    /// them (the renderer's proxy `i` shows body `i`).
    pub bodies: Vec<Body>,
}

impl World {
    pub fn new(bodies: Vec<Body>) -> Self {
        Self { time: 0.0, bodies }
    }

    /// Simulated time since the start, in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advances the whole simulation by exactly `dt` seconds.
    ///
    /// Callers must always pass the same `dt` (the fixed timestep), see
    /// `FixedTimestep` in the app.
    // TODO(step-06): take the pilot's controls (throttle, pitch) as an
    // argument here; physics reads them but never polls a keyboard.
    pub fn step(&mut self, dt: f64) {
        // Each system runs over every body, taking only the components it
        // needs.
        for body in &mut self.bodies {
            // TODO(step-06): add aerodynamic forces (lift, drag) and thrust.
            integrate(&mut body.motion, GRAVITY, dt);
            ground_contact(&mut body.motion, &body.collider);
        }
        self.time += dt;
    }
}

/// Advances a motion by `dt` seconds under a constant `acceleration` (m/s²),
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
pub fn integrate(motion: &mut Motion, acceleration: DVec2, dt: f64) {
    motion.velocity += acceleration * dt;
    motion.position += motion.velocity * dt;
}

/// Keeps something moving with `motion`, with a circular collider
/// `shape`, from going through the ground (y = 0).
///
/// Integration moves in discrete jumps, so a falling ball ends up slightly
/// *inside* the ground on the step it lands. We push it back on top and
/// cancel its downward speed: a perfectly inelastic impact, no bounce.
/// Sideways motion is kept, as there's no friction yet.
// TODO(step-12): replace with landing gear contacts: spring-damper forces
// at each wheel, plus rolling friction and brakes.
pub fn ground_contact(motion: &mut Motion, shape: &Circle) {
    if motion.position.y < shape.radius {
        motion.position.y = shape.radius;
        motion.velocity.y = motion.velocity.y.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 120.0;
    const RADIUS: f64 = 0.4;
    const BALL: Circle = Circle { radius: RADIUS };
    /// g, as a positive number, in m/s².
    const G: f64 = -GRAVITY.y;

    /// 20 m up, thrown sideways at 5 m/s.
    fn thrown() -> Motion {
        Motion {
            position: DVec2::new(0.0, 20.0),
            velocity: DVec2::new(5.0, 0.0),
        }
    }

    /// A ball moving like that.
    fn thrown_ball() -> Body {
        Body {
            motion: thrown(),
            collider: BALL,
        }
    }

    /// A ball of `radius` lying still on the ground at `x`.
    fn resting_ball(x: f64, radius: f64) -> Body {
        Body {
            motion: Motion {
                position: DVec2::new(x, radius),
                velocity: DVec2::ZERO,
            },
            collider: Circle { radius },
        }
    }

    /// Integrates `motion` under gravity alone (no ground) for `seconds`.
    fn fly(mut motion: Motion, seconds: f64, dt: f64) -> Motion {
        let steps = (seconds / dt).round() as u32;
        for _ in 0..steps {
            integrate(&mut motion, GRAVITY, dt);
        }
        motion
    }

    #[test]
    fn gravity_points_down_with_standard_value() {
        assert_eq!(GRAVITY, DVec2::new(0.0, -9.80665));
    }

    #[test]
    fn without_acceleration_a_body_moves_at_constant_velocity() {
        let mut motion = thrown();
        integrate(&mut motion, DVec2::ZERO, 0.5);
        assert_eq!(motion.position, DVec2::new(2.5, 20.0));
        assert_eq!(motion.velocity, DVec2::new(5.0, 0.0));
    }

    #[test]
    fn velocity_is_updated_before_position() {
        // Semi-implicit Euler: the position update already uses the *new*
        // velocity. (Explicit Euler would use the old one and leave the
        // body at y = 0 after this first step.)
        let mut motion = Motion {
            position: DVec2::ZERO,
            velocity: DVec2::ZERO,
        };
        integrate(&mut motion, DVec2::new(0.0, -10.0), 0.5);
        assert_eq!(motion.velocity, DVec2::new(0.0, -5.0));
        assert_eq!(motion.position, DVec2::new(0.0, -2.5));
    }

    #[test]
    fn free_fall_matches_the_analytic_solution_up_to_the_known_euler_error() {
        // Exact:  y(t) = y0 - g t²/2,   vy(t) = -g t.
        // Semi-implicit Euler with a constant acceleration gives exactly
        //   y_n = y0 - g t²/2 - g t dt/2,
        // i.e. it falls g·t·dt/2 too far (4 cm here). The error is
        // proportional to dt: the method is "first-order accurate".
        let t = 1.0;
        let motion = fly(thrown(), t, DT);
        let exact_y = 20.0 - 0.5 * G * t * t;
        let euler_error = 0.5 * G * t * DT;
        assert!((motion.position.y - (exact_y - euler_error)).abs() < 1e-9);
        // Velocity under constant acceleration is exact (up to rounding).
        assert!((motion.velocity.y - (-G * t)).abs() < 1e-9);
    }

    #[test]
    fn halving_the_timestep_halves_the_error() {
        let exact_y = 20.0 - 0.5 * G;
        let error = |dt: f64| (fly(thrown(), 1.0, dt).position.y - exact_y).abs();
        let ratio = error(DT) / error(DT / 2.0);
        assert!((ratio - 2.0).abs() < 1e-6, "ratio = {ratio}");
    }

    #[test]
    fn gravity_does_not_change_horizontal_motion() {
        let motion = fly(thrown(), 1.0, DT);
        assert!((motion.position.x - 5.0).abs() < 1e-9);
        assert_eq!(motion.velocity.x, 5.0);
    }

    #[test]
    fn ground_contact_puts_a_sunken_ball_back_on_the_ground() {
        let mut motion = Motion {
            position: DVec2::new(3.0, -0.1),
            velocity: DVec2::new(5.0, -2.0),
        };
        ground_contact(&mut motion, &BALL);
        assert_eq!(motion.position, DVec2::new(3.0, RADIUS));
        // Falling stops; no friction yet, so it keeps sliding sideways.
        assert_eq!(motion.velocity, DVec2::new(5.0, 0.0));
    }

    #[test]
    fn ground_contact_ignores_a_ball_in_the_air() {
        let mut motion = thrown();
        ground_contact(&mut motion, &BALL);
        assert_eq!(motion, thrown());
    }

    #[test]
    fn world_starts_at_time_zero_with_its_bodies() {
        let world = World::new(vec![thrown_ball(), resting_ball(5.0, 1.0)]);
        assert_eq!(world.time(), 0.0);
        assert_eq!(world.bodies, [thrown_ball(), resting_ball(5.0, 1.0)]);
    }

    #[test]
    fn ball_lands_at_the_free_fall_time_and_stays_on_the_ground() {
        // Its centre falls h = 20 m - radius, which takes t = sqrt(2h/g).
        let impact = (2.0 * (20.0 - RADIUS) / G).sqrt();
        let mut world = World::new(vec![thrown_ball()]);
        let height = |world: &World| world.bodies[0].motion.position.y;

        while world.time() < impact - 2.0 * DT {
            world.step(DT);
        }
        assert!(height(&world) > RADIUS, "landed too early");

        while world.time() < impact + 2.0 * DT {
            world.step(DT);
        }
        assert_eq!(height(&world), RADIUS, "not landed in time");
        assert_eq!(world.bodies[0].motion.velocity.y, 0.0);

        for _ in 0..600 {
            world.step(DT); // 5 more seconds: it must not sink or bounce
        }
        assert_eq!(height(&world), RADIUS);
    }

    #[test]
    fn every_body_is_simulated_with_its_own_collider() {
        let mut world = World::new(vec![thrown_ball(), resting_ball(5.0, 1.0)]);
        for _ in 0..120 {
            world.step(DT);
        }
        // The thrown ball fell; the big one, resting on its 1 m radius,
        // stayed put.
        assert!(world.bodies[0].motion.position.y < 20.0);
        assert_eq!(world.bodies[1], resting_ball(5.0, 1.0));
    }

    #[test]
    fn simulation_is_deterministic() {
        // Two worlds fed the same steps must be bit-identical (assert_eq!,
        // not a tolerance). Replays in step 10 rely on this.
        let bodies = vec![thrown_ball(), resting_ball(5.0, 1.0)];
        let mut a = World::new(bodies.clone());
        let mut b = World::new(bodies);
        for _ in 0..1000 {
            a.step(DT);
            b.step(DT);
        }
        assert_eq!(a, b);
    }
}
