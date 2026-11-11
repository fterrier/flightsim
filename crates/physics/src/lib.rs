//! Flight physics: pure, deterministic simulation code.
//!
//! Rules for this crate (see ROADMAP.md):
//! - No I/O, no clock, no graphics. Time only enters as a `dt` argument.
//! - Same inputs => same outputs, bit for bit. This is what makes tests
//!   and (later) replays possible.
//! - SI units everywhere: metres, seconds, kilograms, radians.
//!
//!
//! Coordinates: **NED**, as in flight-dynamics textbooks (Stevens & Lewis,
//! Beard & McLain): x points **N**orth, y **E**ast, z **D**own, origin on
//! the ground. So *altitude is -z*, and a climbing body has a negative
//! vertical velocity. Right-handed: north × east = down. The renderer
//! converts to its own axes in exactly one place (see ROADMAP "Axes").

/// Re-exported so the engine (and, through it, games) use exactly the same
/// vector types as the physics. `DVec2` is handy for horizontal (north,
/// east) quantities.
pub use glam::{DVec2, DVec3};

/// Standard gravity at the Earth's surface, in m/s², pointing down (+z in
/// NED): the usual value for a [`World`]'s `gravity`.
///
/// It's an *acceleration*, the same for every body whatever its mass
/// (Galileo), so a falling ball needs no mass yet.
pub const STANDARD_GRAVITY: DVec3 = DVec3::new(0.0, 0.0, 9.80665);

/// How something moves: where it is and how fast it goes.
///
/// Plain data, no behaviour: in ECS terms this is a *component*.
/// Behaviour lives in free functions such as [`integrate`] (a *system*).
// TODO(step-06): mass arrives with forces (acceleration = force / mass).
// TODO(step-08): orientation and angular velocity (rigid body).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Position (NED), in metres.
    pub position: DVec3,
    /// Velocity (NED), in metres per second.
    pub velocity: DVec3,
}

/// Whether a body moves. As in physics engines, the ground is a body like
/// any other, one that never moves: Rapier's `RigidBodyType::Fixed`, Jolt's
/// `EMotionType::Static`, Box2D's `b2_staticBody`, PhysX's `PxRigidStatic`.
/// One list of bodies, one index for each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyType {
    /// Moves under gravity (later forces) and is stopped by static bodies.
    Dynamic,
    /// Never moves: its velocity is ignored. The ground, later buildings.
    Static,
}

/// A collision shape, placed at its body's position.
// TODO(step-12): boxes for buildings, heightfields for real terrain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collider {
    /// A sphere of `radius` metres, centred on the body.
    Sphere { radius: f64 },
    /// Everything below a horizontal plane through the body's position:
    /// an endless flat ground. Rapier calls it a half-space, Jolt a plane
    /// shape, Godot a world boundary.
    HalfSpace,
}

/// A simulated object: whether it moves, how, and what it bumps into.
///
/// Every physics engine attaches colliders to a body (Box2D creates shapes
/// on a body, Rapier inserts colliders "with parent", Unity puts colliders
/// on the Rigidbody's GameObject): the collider moves with its body. Kept
/// as separate components so systems can take only what they need.
// TODO(step-12): several colliders per body (fuselage, wings, wheels), as
// engines' "compound colliders".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub body_type: BodyType,
    pub motion: Motion,
    pub collider: Collider,
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
    /// Acceleration applied to every dynamic body, in m/s². A setting of
    /// the world, as in Rapier (`gravity` passed to each step) or Box2D
    /// (`b2WorldDef`): the engine assumes no particular planet, nor which
    /// way is down.
    pub gravity: DVec3,
    /// All simulated objects, moving and static, stored contiguously.
    /// Their index identifies them: the world has no built-in ground,
    /// whatever ground exists is one of these bodies.
    pub bodies: Vec<Body>,
}

impl World {
    pub fn new(bodies: Vec<Body>, gravity: DVec3) -> Self {
        Self {
            time: 0.0,
            gravity,
            bodies,
        }
    }

    /// Simulated time since the start, in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advances the whole simulation by exactly `dt` seconds.
    ///
    /// Callers must always pass the same `dt` (the fixed timestep), see
    /// `FixedTimestep` in the engine.
    // TODO(step-06): take the pilot's controls (throttle, pitch) as an
    // argument here; physics reads them but never polls a keyboard.
    pub fn step(&mut self, dt: f64) {
        // Each system runs over every body, taking only the components it
        // needs. Indices, not iterators: a body is moved while reading
        // others in the same list.
        for i in 0..self.bodies.len() {
            if self.bodies[i].body_type == BodyType::Static {
                continue;
            }
            // TODO(step-06): add aerodynamic forces (lift, drag) and thrust.
            integrate(&mut self.bodies[i].motion, self.gravity, dt);
            // Every pair of shapes checked: fine for a handful of bodies.
            // TODO(step-12): collisions between all shapes (balls bumping
            // into each other), with a broad phase to skip far-apart pairs.
            for j in 0..self.bodies.len() {
                // A copy (a few dozen bytes on the stack), so `bodies[i]`
                // can be borrowed mutably next.
                let other = self.bodies[j];
                let body = &mut self.bodies[i];
                if let (BodyType::Static, Collider::HalfSpace, Collider::Sphere { radius }) =
                    (other.body_type, other.collider, body.collider)
                {
                    let elevation = -other.motion.position.z;
                    ground_contact(&mut body.motion, radius, elevation);
                }
            }
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
fn integrate(motion: &mut Motion, acceleration: DVec3, dt: f64) {
    motion.velocity += acceleration * dt;
    motion.position += motion.velocity * dt;
}

/// Keeps something moving with `motion`, a sphere of `radius` metres, from
/// going through a flat ground at `elevation` metres (the plane
/// z = -elevation; z is down, so "below the ground" means a larger z).
///
/// Integration moves in discrete jumps, so a falling ball ends up slightly
/// *inside* the ground on the step it lands. We push it back on top and
/// cancel its downward speed: a perfectly inelastic impact, no bounce.
/// Sideways motion is kept, as there's no friction yet.
// TODO(step-12): replace with landing gear contacts: spring-damper forces
// at each wheel, plus rolling friction and brakes.
fn ground_contact(motion: &mut Motion, radius: f64, elevation: f64) {
    // The centre must stay `radius` above the ground.
    let lowest = -elevation - radius;
    if motion.position.z > lowest {
        motion.position.z = lowest;
        // Cancel downward (+z) speed, keep any upward (-z) speed.
        motion.velocity.z = motion.velocity.z.min(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 120.0;
    const RADIUS: f64 = 0.4;
    /// g, as a positive number, in m/s².
    const G: f64 = 9.80665;

    /// Flat ground `elevation` metres up (z is down: at z = -elevation).
    fn ground(elevation: f64) -> Body {
        Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 0.0, -elevation),
                velocity: DVec3::ZERO,
            },
            collider: Collider::HalfSpace,
        }
    }

    /// 20 m above the ground (z is *down*, so z = -20), thrown north at
    /// 5 m/s.
    fn thrown() -> Motion {
        Motion {
            position: DVec3::new(0.0, 0.0, -20.0),
            velocity: DVec3::new(5.0, 0.0, 0.0),
        }
    }

    /// A ball moving like that.
    fn thrown_ball() -> Body {
        Body {
            body_type: BodyType::Dynamic,
            motion: thrown(),
            collider: Collider::Sphere { radius: RADIUS },
        }
    }

    /// A ball of `radius` lying still on the ground, `north` metres north.
    fn resting_ball(north: f64, radius: f64) -> Body {
        Body {
            body_type: BodyType::Dynamic,
            motion: Motion {
                position: DVec3::new(north, 0.0, -radius),
                velocity: DVec3::ZERO,
            },
            collider: Collider::Sphere { radius },
        }
    }

    /// Integrates `motion` under gravity alone (no ground) for `seconds`.
    fn fly(mut motion: Motion, seconds: f64, dt: f64) -> Motion {
        let steps = (seconds / dt).round() as u32;
        for _ in 0..steps {
            integrate(&mut motion, STANDARD_GRAVITY, dt);
        }
        motion
    }

    #[test]
    fn gravity_points_down_which_is_plus_z_in_ned() {
        assert_eq!(STANDARD_GRAVITY, DVec3::new(0.0, 0.0, G));
    }

    #[test]
    fn ned_axes_are_right_handed() {
        // North × East = Down, like x × y = z: the usual maths (cross
        // products, rotations) works unchanged.
        assert_eq!(DVec3::X.cross(DVec3::Y), DVec3::Z);
    }

    #[test]
    fn without_acceleration_a_body_moves_at_constant_velocity() {
        let mut motion = thrown();
        integrate(&mut motion, DVec3::ZERO, 0.5);
        assert_eq!(motion.position, DVec3::new(2.5, 0.0, -20.0));
        assert_eq!(motion.velocity, DVec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn velocity_is_updated_before_position() {
        // Semi-implicit Euler: the position update already uses the *new*
        // velocity. (Explicit Euler would use the old one and leave the
        // body where it was after this first step.)
        let mut motion = Motion {
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
        };
        integrate(&mut motion, DVec3::new(0.0, 0.0, 10.0), 0.5);
        assert_eq!(motion.velocity, DVec3::new(0.0, 0.0, 5.0));
        assert_eq!(motion.position, DVec3::new(0.0, 0.0, 2.5));
    }

    #[test]
    fn free_fall_matches_the_analytic_solution_up_to_the_known_euler_error() {
        // Exact (z down):  z(t) = z0 + g t²/2,   vz(t) = g t.
        // Semi-implicit Euler with a constant acceleration gives exactly
        //   z_n = z0 + g t²/2 + g t dt/2,
        // i.e. it falls g·t·dt/2 too far (4 cm here). The error is
        // proportional to dt: the method is "first-order accurate".
        let t = 1.0;
        let motion = fly(thrown(), t, DT);
        let exact_z = -20.0 + 0.5 * G * t * t;
        let euler_error = 0.5 * G * t * DT;
        assert!((motion.position.z - (exact_z + euler_error)).abs() < 1e-9);
        // Velocity under constant acceleration is exact (up to rounding).
        assert!((motion.velocity.z - G * t).abs() < 1e-9);
    }

    #[test]
    fn halving_the_timestep_halves_the_error() {
        let exact_z = -20.0 + 0.5 * G;
        let error = |dt: f64| (fly(thrown(), 1.0, dt).position.z - exact_z).abs();
        let ratio = error(DT) / error(DT / 2.0);
        assert!((ratio - 2.0).abs() < 1e-6, "ratio = {ratio}");
    }

    #[test]
    fn gravity_does_not_change_horizontal_motion() {
        // Thrown north-west: 5 m/s north, 3 m/s west (-east).
        let start = Motion {
            velocity: DVec3::new(5.0, -3.0, 0.0),
            ..thrown()
        };
        let motion = fly(start, 1.0, DT);
        assert!((motion.position.truncate() - DVec2::new(5.0, -3.0)).length() < 1e-9);
        assert_eq!(motion.velocity.truncate(), DVec2::new(5.0, -3.0));
    }

    #[test]
    fn ground_contact_puts_a_sunken_ball_back_on_the_ground() {
        // Its centre is 0.1 m *below* the ground (z = +0.1), falling.
        let mut motion = Motion {
            position: DVec3::new(3.0, 4.0, 0.1),
            velocity: DVec3::new(5.0, 6.0, 2.0),
        };
        ground_contact(&mut motion, RADIUS, 0.0);
        assert_eq!(motion.position, DVec3::new(3.0, 4.0, -RADIUS));
        // Falling stops; no friction yet, so it keeps sliding.
        assert_eq!(motion.velocity, DVec3::new(5.0, 6.0, 0.0));
    }

    #[test]
    fn ground_contact_ignores_a_ball_in_the_air() {
        let mut motion = thrown();
        ground_contact(&mut motion, RADIUS, 0.0);
        assert_eq!(motion, thrown());
    }

    #[test]
    fn ground_contact_respects_the_ground_elevation() {
        // Ground 10 m up (z = -10): the ball's centre stops at z = -10.4.
        let mut motion = Motion {
            position: DVec3::new(0.0, 0.0, -9.0),
            velocity: DVec3::new(0.0, 0.0, 2.0),
        };
        ground_contact(&mut motion, RADIUS, 10.0);
        assert_eq!(motion.position.z, -10.0 - RADIUS);
        assert_eq!(motion.velocity.z, 0.0);
    }

    #[test]
    fn without_a_ground_bodies_fall_forever() {
        // The ground is not built in: it's a static body the world may or
        // may not have (like a Rapier fixed body or a Godot StaticBody3D),
        // placed by the scene.
        let mut world = World::new(vec![thrown_ball()], STANDARD_GRAVITY);
        for _ in 0..600 {
            world.step(DT);
        }
        assert!(
            world.bodies[0].motion.position.z > 100.0,
            "fell through: no ground"
        );
    }

    #[test]
    fn a_raised_ground_holds_bodies_at_its_elevation() {
        let mut world = World::new(vec![ground(10.0), thrown_ball()], STANDARD_GRAVITY);
        for _ in 0..600 {
            world.step(DT);
        }
        let altitude = -world.bodies[1].motion.position.z;
        assert_eq!(altitude, 10.0 + RADIUS);
    }

    #[test]
    fn static_bodies_never_move() {
        // Not even with a velocity, under gravity: static means static.
        let mut moving_ground = ground(0.0);
        moving_ground.motion.velocity = DVec3::new(1.0, 2.0, 3.0);
        let mut world = World::new(vec![moving_ground], STANDARD_GRAVITY);
        for _ in 0..120 {
            world.step(DT);
        }
        assert_eq!(world.bodies, [moving_ground]);
    }

    #[test]
    fn every_static_ground_applies() {
        // Two grounds: the higher one wins, as the ball lands on it first.
        let bodies = vec![ground(0.0), thrown_ball(), ground(5.0)];
        let mut world = World::new(bodies, STANDARD_GRAVITY);
        for _ in 0..600 {
            world.step(DT);
        }
        assert_eq!(-world.bodies[1].motion.position.z, 5.0 + RADIUS);
    }

    #[test]
    fn gravity_is_a_setting_of_the_world() {
        // Rapier and Box2D make gravity a world parameter too: the engine
        // assumes no planet. Without gravity, a thrown ball flies straight.
        let mut world = World::new(vec![thrown_ball(), ground(0.0)], DVec3::ZERO);
        for _ in 0..120 {
            world.step(DT);
        }
        assert_eq!(world.bodies[0].motion.velocity, DVec3::new(5.0, 0.0, 0.0));
        let expected = DVec3::new(5.0, 0.0, -20.0);
        assert!((world.bodies[0].motion.position - expected).length() < 1e-9);

        // Gravity can point anywhere: westwards, it bends the path west.
        world.gravity = DVec3::new(0.0, -2.0, 0.0);
        world.step(1.0);
        assert_eq!(world.bodies[0].motion.velocity, DVec3::new(5.0, -2.0, 0.0));
    }

    #[test]
    fn world_starts_at_time_zero_with_its_bodies() {
        let bodies = vec![ground(0.0), thrown_ball(), resting_ball(5.0, 1.0)];
        let world = World::new(bodies.clone(), STANDARD_GRAVITY);
        assert_eq!(world.time(), 0.0);
        assert_eq!(world.bodies, bodies);
    }

    #[test]
    fn ball_lands_at_the_free_fall_time_and_stays_on_the_ground() {
        // Its centre falls h = 20 m - radius, which takes t = sqrt(2h/g).
        let impact = (2.0 * (20.0 - RADIUS) / G).sqrt();
        let mut world = World::new(vec![thrown_ball(), ground(0.0)], STANDARD_GRAVITY);
        let altitude = |world: &World| -world.bodies[0].motion.position.z;

        while world.time() < impact - 2.0 * DT {
            world.step(DT);
        }
        assert!(altitude(&world) > RADIUS, "landed too early");

        while world.time() < impact + 2.0 * DT {
            world.step(DT);
        }
        assert_eq!(altitude(&world), RADIUS, "not landed in time");
        assert_eq!(world.bodies[0].motion.velocity.z, 0.0);

        for _ in 0..600 {
            world.step(DT); // 5 more seconds: it must not sink or bounce
        }
        assert_eq!(altitude(&world), RADIUS);
    }

    #[test]
    fn every_body_is_simulated_with_its_own_collider() {
        let bodies = vec![thrown_ball(), resting_ball(5.0, 1.0), ground(0.0)];
        let mut world = World::new(bodies, STANDARD_GRAVITY);
        for _ in 0..120 {
            world.step(DT);
        }
        // The thrown ball fell; the big one, resting on its 1 m radius,
        // stayed put. (They pass through each other: only static grounds
        // stop bodies so far.)
        assert!(world.bodies[0].motion.position.z > -20.0);
        assert_eq!(world.bodies[1], resting_ball(5.0, 1.0));
    }

    #[test]
    fn simulation_is_deterministic() {
        // Two worlds fed the same steps must be bit-identical (assert_eq!,
        // not a tolerance). Replays in step 10 rely on this.
        let bodies = vec![ground(0.0), thrown_ball(), resting_ball(5.0, 1.0)];
        let mut a = World::new(bodies.clone(), STANDARD_GRAVITY);
        let mut b = World::new(bodies, STANDARD_GRAVITY);
        for _ in 0..1000 {
            a.step(DT);
            b.step(DT);
        }
        assert_eq!(a, b);
    }
}
