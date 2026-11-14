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

use std::fmt;

use arena::Arena;

/// Standard gravity at the Earth's surface, in m/s², pointing down (+z in
/// NED): the usual value for a [`World`]'s `gravity`.
///
/// It's an *acceleration*, the same for every body whatever its mass
/// (Galileo).
pub const STANDARD_GRAVITY: DVec3 = DVec3::new(0.0, 0.0, 9.80665);

/// How something moves: where it is and how fast it goes.
///
/// Plain data, no behaviour: in ECS terms this is a *component*.
/// Behaviour lives in free functions such as [`integrate`] (a *system*).
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
/// One collection of bodies, one ID for each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyType {
    /// Moves under gravity and the forces applied to it, and is stopped by
    /// static bodies. Its `mass` (kg, positive) says how hard it is to
    /// push: acceleration = force / mass.
    Dynamic { mass: f64 },
    /// Never moves, whatever pushes it, as if infinitely heavy: so it has
    /// no mass, and its velocity is ignored. The ground, later buildings.
    Static,
}

impl BodyType {
    pub fn is_dynamic(self) -> bool {
        matches!(self, BodyType::Dynamic { .. })
    }
}

/// A collision shape, placed at its body's position.
// TODO(step-12): boxes for buildings, heightfields for real terrain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collider {
    /// A sphere of `radius` metres, centred on the body.
    Sphere { radius: f64 },
    /// Everything below a horizontal plane through the body's position:
    /// an endless flat ground. Rapier calls it a half-space, Jolt a plane
    /// shape, Godot a world boundary. Static bodies only.
    HalfSpace,
}

impl Collider {
    /// Whether only a static body can have this shape. An endless
    /// half-space has no finite mass or centre: it can't be moved by
    /// forces. Jolt has the same check (`Shape::MustBeStatic`; "planes
    /// cannot be dynamic objects").
    pub fn must_be_static(self) -> bool {
        match self {
            Collider::Sphere { .. } => false,
            Collider::HalfSpace => true,
        }
    }
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
    /// Bounciness, from 0 (dead: an impact stops it, like clay) to 1
    /// (perfectly elastic: no energy lost, like an ideal billiard ball).
    /// The *coefficient of restitution*: how much of the speed of approach
    /// is given back as speed of separation. Per pair, the smaller of the
    /// two bodies' values counts (see `restitution`).
    // TODO(step-12): per collider (engines put it on the collider or its
    // material), once a body has several.
    pub restitution: f64,
}

/// The restitution of a collision between two bodies: the smaller of
/// theirs. Engines offer several combine rules (Rapier averages by
/// default, Box2D takes the larger, Unity and Rapier also offer the
/// smaller); the smaller keeps a dead ground (0) dead under any ball,
/// with no need for the "no bounce below some speed" threshold engines add
/// to stop resting objects from jittering.
fn restitution(a: &Body, b: &Body) -> f64 {
    a.restitution.min(b.restitution)
}

/// 1 / mass, in 1/kg: how much an impulse changes a body's velocity.
/// Static bodies are infinitely heavy: 0, an impulse doesn't move them.
/// This lets one collision response handle moving and static bodies alike,
/// as engines do.
fn inverse_mass(body_type: BodyType) -> f64 {
    match body_type {
        BodyType::Dynamic { mass } => 1.0 / mass,
        BodyType::Static => 0.0,
    }
}

/// How many times per step the contacts' velocity impulses are solved.
/// Each pass lets a push travel one more ball along a packed row, so a
/// chain reaction crosses a rack within a step or two, as engines do with
/// their "velocity iterations" (Box2D, Rapier). More: better chains,
/// slower steps.
const SOLVER_ITERATIONS: usize = 8;

/// Room for this many contacts per body is set aside as bodies join the
/// world, so steps don't allocate: a ball in a packed rack touches its
/// neighbours and the ground. Busier piles make the buffer grow (once).
const CONTACTS_PER_BODY: usize = 8;

arena::id_type! {
    /// Identifies a body of a [`World`]: handed out by [`World::insert`],
    /// like Rapier's `RigidBodyHandle`. Opaque and generational (see the
    /// `arena` crate).
    pub struct BodyId;
}

/// A body as the world stores it: the body, and the forces pushing it
/// during the coming step.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    body: Body,
    /// Newtons (NED), accumulated by `add_force`, cleared after each step.
    force: DVec3,
}

/// Everything the simulation knows about.
// TODO(step-11): bodies become ECS components; systems like `integrate`
// won't change: they take components.
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
    /// All simulated objects, moving and static, by ID. The world has no
    /// built-in ground: whatever ground exists is one of these bodies.
    bodies: Arena<BodyId, Entry>,
    /// Every body's ID, in insertion order: what the collision pass goes
    /// through, since it modifies bodies while reading others (so it can't
    /// iterate the arena itself). Kept as bodies are inserted, so stepping
    /// never allocates.
    ids: Vec<BodyId>,
    /// The contacts found during the current step. Kept between steps so
    /// its memory is reused: cleared, never freed.
    touching: Vec<Touching>,
}

/// Two bodies found touching during a step, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Touching {
    a: BodyId,
    b: BodyId,
    /// Seen from `a`: the normal points the way `a` must move to get out.
    contact: Contact,
    restitution: f64,
}

/// Why a body can't join a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldError {
    /// The body is dynamic, but its collider can only be static (see
    /// [`Collider::must_be_static`]).
    MustBeStatic,
    /// The body is dynamic, but its mass isn't a positive number of
    /// kilograms: no force could move it sensibly.
    InvalidMass,
    /// The body's restitution isn't between 0 and 1: it would gain energy
    /// in every collision, or make no sense.
    InvalidRestitution,
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::MustBeStatic => {
                write!(f, "a dynamic body's collider can only be static")
            }
            WorldError::InvalidMass => write!(f, "a dynamic body's mass isn't positive"),
            WorldError::InvalidRestitution => write!(f, "a body's restitution isn't within 0..=1"),
        }
    }
}

impl std::error::Error for WorldError {}

impl World {
    /// An empty world at time zero.
    pub fn new(gravity: DVec3) -> Self {
        Self {
            time: 0.0,
            gravity,
            bodies: Arena::new(),
            ids: Vec::new(),
            touching: Vec::new(),
        }
    }

    /// Adds `body` and returns its new ID, if it makes sense (see
    /// [`WorldError`]): checked once here, so `step` never meets a body it
    /// can't simulate.
    pub fn insert(&mut self, body: Body) -> Result<BodyId, WorldError> {
        if !(0.0..=1.0).contains(&body.restitution) {
            return Err(WorldError::InvalidRestitution);
        }
        if let BodyType::Dynamic { mass } = body.body_type {
            if body.collider.must_be_static() {
                return Err(WorldError::MustBeStatic);
            }
            // A finite number (not NaN, not infinite) above zero.
            let valid = mass.is_finite() && mass > 0.0;
            if !valid {
                return Err(WorldError::InvalidMass);
            }
        }
        let entry = Entry {
            body,
            force: DVec3::ZERO,
        };
        let id = self.bodies.insert(entry);
        self.ids.push(id);
        self.touching.reserve(CONTACTS_PER_BODY);
        Ok(id)
    }

    /// The body with this ID, if it's in this world.
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id).map(|entry| &entry.body)
    }

    /// Simulated time since the start, in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Pushes body `id` with `force` (newtons, NED) during the next step,
    /// on top of any other force added for it. Cleared after the step, so
    /// a steady push is added again every step (Rapier's `add_force`,
    /// Box2D's `b2Body_ApplyForce`). The physics never knows where forces
    /// come from (a player's keys, an engine's thrust, wings' lift). A body
    /// not in this world is ignored; static bodies ignore forces anyway.
    pub fn add_force(&mut self, id: BodyId, force: DVec3) {
        if let Some(entry) = self.bodies.get_mut(id) {
            entry.force += force;
        }
    }

    /// Advances the whole simulation by exactly `dt` seconds.
    ///
    /// Callers must always pass the same `dt` (the fixed timestep), see
    /// `FixedTimestep` in the engine.
    pub fn step(&mut self, dt: f64) {
        // Each system runs over every body, taking only the components it
        // needs. First everything moves...
        for (_, entry) in self.bodies.iter_mut() {
            if let BodyType::Dynamic { mass } = entry.body.body_type {
                // Newton's second law: a = F / m, plus gravity (the same
                // acceleration for any mass).
                let acceleration = self.gravity + entry.force / mass;
                integrate(&mut entry.body.motion, acceleration, dt);
            }
            entry.force = DVec3::ZERO;
        }

        // ...then collisions are found and resolved, in separate stages as
        // in every physics engine: *detection* (`contact`, per pair of
        // shapes) says which bodies touch and how; *response* acts on
        // that, whatever the shapes: first velocities (`bounce`), then
        // positions (`separate`).
        //
        // Every pair is tried, fine for a hundred bodies or so.
        // TODO(step-12): a broad phase, to skip far-apart pairs without
        // testing them (the many-balls stress test measures when it pays).
        self.find_contacts();
        for _ in 0..SOLVER_ITERATIONS {
            for touching in &self.touching {
                Self::bounce_pair(&mut self.bodies, touching);
            }
        }
        for touching in &self.touching {
            Self::separate_pair(&mut self.bodies, touching);
        }
        self.time += dt;
    }

    /// Detection: fills `touching` with every pair of bodies that overlap,
    /// at least one of them moving (two static bodies never collide). By
    /// ID, in insertion order: the same order every time (determinism).
    fn find_contacts(&mut self) {
        self.touching.clear();
        for (i, &a) in self.ids.iter().enumerate() {
            for &b in &self.ids[i + 1..] {
                let (Some(first), Some(second)) = (self.bodies.get(a), self.bodies.get(b)) else {
                    continue;
                };
                let (first, second) = (&first.body, &second.body);
                if !first.body_type.is_dynamic() && !second.body_type.is_dynamic() {
                    continue;
                }
                let (at_a, at_b) = (first.motion.position, second.motion.position);
                if let Some(contact) = contact(first.collider, at_a, second.collider, at_b) {
                    let restitution = restitution(first, second);
                    self.touching.push(Touching {
                        a,
                        b,
                        contact,
                        restitution,
                    });
                }
            }
        }
    }

    /// Velocity response to one contact (see `bounce`), written back to
    /// the moving bodies only: static ones never change (and aren't
    /// written, as Box2D avoids writing them).
    // TODO(step-11): read and write only velocities and inverse masses, in
    // packed arrays, instead of copying motions in and out by ID.
    fn bounce_pair(bodies: &mut Arena<BodyId, Entry>, touching: &Touching) {
        let (Some(a), Some(b)) = (bodies.get(touching.a), bodies.get(touching.b)) else {
            return;
        };
        let (inverse_a, inverse_b) = (
            inverse_mass(a.body.body_type),
            inverse_mass(b.body.body_type),
        );
        // A static body's velocity is ignored: it doesn't move.
        let mut velocity_a = moving_velocity(&a.body);
        let mut velocity_b = moving_velocity(&b.body);
        bounce(
            (&mut velocity_a, inverse_a),
            (&mut velocity_b, inverse_b),
            touching.contact.normal,
            touching.restitution,
        );
        for (id, velocity) in [(touching.a, velocity_a), (touching.b, velocity_b)] {
            if let Some(entry) = bodies.get_mut(id).filter(|e| e.body.body_type.is_dynamic()) {
                entry.body.motion.velocity = velocity;
            }
        }
    }

    /// Position response to one contact (see `separate`), written back to
    /// the moving bodies only.
    fn separate_pair(bodies: &mut Arena<BodyId, Entry>, touching: &Touching) {
        let (Some(a), Some(b)) = (bodies.get(touching.a), bodies.get(touching.b)) else {
            return;
        };
        let (inverse_a, inverse_b) = (
            inverse_mass(a.body.body_type),
            inverse_mass(b.body.body_type),
        );
        let mut position_a = a.body.motion.position;
        let mut position_b = b.body.motion.position;
        separate(
            (&mut position_a, inverse_a),
            (&mut position_b, inverse_b),
            touching.contact,
        );
        for (id, position) in [(touching.a, position_a), (touching.b, position_b)] {
            if let Some(entry) = bodies.get_mut(id).filter(|e| e.body.body_type.is_dynamic()) {
                entry.body.motion.position = position;
            }
        }
    }
}

/// A body's velocity as collisions see it: a static body doesn't move,
/// whatever velocity it was given.
fn moving_velocity(body: &Body) -> DVec3 {
    if body.body_type.is_dynamic() {
        body.motion.velocity
    } else {
        DVec3::ZERO
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

/// Where two colliders overlap: what collision detection reports, and all
/// that collision response needs, whatever the shapes (as Parry's
/// `Contact`, used by Rapier).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Contact {
    /// Unit vector (NED) along which the first collider must move to get
    /// out of the second.
    normal: DVec3,
    /// How deep they overlap, in metres (positive).
    depth: f64,
}

impl Contact {
    /// The same contact seen from the other collider.
    fn flipped(self) -> Self {
        Self {
            normal: -self.normal,
            ..self
        }
    }
}

/// Collision detection (the "narrow phase"): whether collider `a`, placed
/// at `at_a`, overlaps collider `b`, placed at `at_b` (NED, metres), and
/// how. One function per pair of shape types.
fn contact(a: Collider, at_a: DVec3, b: Collider, at_b: DVec3) -> Option<Contact> {
    match (a, b) {
        (Collider::Sphere { radius: ra }, Collider::Sphere { radius: rb }) => {
            sphere_sphere(at_a, ra, at_b, rb)
        }
        (Collider::Sphere { radius }, Collider::HalfSpace) => sphere_half_space(at_a, radius, at_b),
        (Collider::HalfSpace, Collider::Sphere { radius }) => {
            sphere_half_space(at_b, radius, at_a).map(Contact::flipped)
        }
        // Never asked by `step`: both would be static (`World::new`
        // checks it).
        (Collider::HalfSpace, Collider::HalfSpace) => None,
    }
}

/// A sphere of `radius` centred at `centre`, against a half-space whose
/// surface passes through `surface` (NED, metres).
fn sphere_half_space(centre: DVec3, radius: f64, surface: DVec3) -> Option<Contact> {
    // z is down: the centre is `surface.z - centre.z` above the surface.
    let height = surface.z - centre.z;
    let depth = radius - height;
    (depth > 0.0).then_some(Contact {
        normal: DVec3::NEG_Z, // up
        depth,
    })
}

/// Two spheres, of radii `ra` and `rb`, centred at `a` and `b` (NED,
/// metres).
fn sphere_sphere(a: DVec3, ra: f64, b: DVec3, rb: f64) -> Option<Contact> {
    let apart = a - b;
    let distance = apart.length();
    let depth = ra + rb - distance;
    // Exactly the same centre: any direction would do; up is as good as
    // any (and gets a ball out of a static sphere on the ground).
    let normal = apart.try_normalize().unwrap_or(DVec3::NEG_Z);
    (depth > 0.0).then_some(Contact { normal, depth })
}

/// Velocity response to a contact, with an *impulse*: an instant change
/// of momentum, along the contact `normal` (pointing the way `a` must move
/// to get out of `b`). `a` and `b` are each a velocity (m/s, NED) and an
/// inverse mass (1/kg; 0 for a static body, which doesn't move).
///
/// The textbook impulse (Box2D's and Rapier's contacts work the same way):
/// if the bodies are approaching along the normal at speed `v`, the
/// impulse `j = (1 + e) v / (1/m_a + 1/m_b)` turns that approach into a
/// separation `e` times as fast, sharing the change by inverse mass: the
/// lighter body changes most. Momentum is conserved (equal and opposite
/// impulses); kinetic energy too when `e = 1`. Sliding along the surface
/// is kept, as there's no friction yet. Bodies already separating are left
/// alone.
// TODO(step-12): spring-damper forces for landing gear, with rolling
// friction and brakes.
fn bounce(a: (&mut DVec3, f64), b: (&mut DVec3, f64), normal: DVec3, restitution: f64) {
    let ((velocity_a, inverse_a), (velocity_b, inverse_b)) = (a, b);
    let total = inverse_a + inverse_b;
    // Speed of `a` relative to `b`, along the normal: negative when
    // approaching.
    let along = (*velocity_a - *velocity_b).dot(normal);
    if along >= 0.0 || total == 0.0 {
        return;
    }
    let impulse = -(1.0 + restitution) * along / total;
    *velocity_a += normal * (impulse * inverse_a);
    *velocity_b -= normal * (impulse * inverse_b);
}

/// Position response to a contact: pushes two overlapping bodies apart by
/// the contact's depth, along its normal, shared by inverse mass (a static
/// body doesn't move; two equal balls each move half). `a` and `b` are
/// each a position (m, NED) and an inverse mass.
///
/// Integration moves in discrete jumps, so bodies end up slightly *inside*
/// each other on the step they meet: the overlap is the contact's depth.
fn separate(a: (&mut DVec3, f64), b: (&mut DVec3, f64), contact: Contact) {
    let ((position_a, inverse_a), (position_b, inverse_b)) = (a, b);
    let total = inverse_a + inverse_b;
    if total == 0.0 {
        return;
    }
    *position_a += contact.normal * (contact.depth * inverse_a / total);
    *position_b -= contact.normal * (contact.depth * inverse_b / total);
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
            restitution: 0.0,
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
            body_type: BodyType::Dynamic { mass: 1.0 },
            motion: thrown(),
            collider: Collider::Sphere { radius: RADIUS },
            restitution: 0.0,
        }
    }

    /// A ball of `radius` lying still on the ground, `north` metres north.
    fn resting_ball(north: f64, radius: f64) -> Body {
        Body {
            body_type: BodyType::Dynamic { mass: 1.0 },
            motion: Motion {
                position: DVec3::new(north, 0.0, -radius),
                velocity: DVec3::ZERO,
            },
            collider: Collider::Sphere { radius },
            restitution: 0.0,
        }
    }

    /// A world of valid `bodies` (tests build it from known-good bodies).
    /// Puts `body` into `world`, which must accept it: its new ID.
    fn add(world: &mut World, body: Body) -> BodyId {
        world.insert(body).expect("a valid body")
    }

    /// Body `id` of `world`, which must be there.
    fn get(world: &World, id: BodyId) -> Body {
        *world.body(id).expect("in the world")
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

    /// Positions after a collision response are compared within 1e-12 m:
    /// the push-out is computed from the overlap depth, a difference of
    /// nearly equal numbers, which rounds in the last bits.
    const PUSH_TOLERANCE: f64 = 1e-12;

    const SPHERE: Collider = Collider::Sphere { radius: RADIUS };
    const BIG_SPHERE: Collider = Collider::Sphere { radius: 1.0 };

    #[test]
    fn a_sunken_ball_touches_the_ground_with_an_upward_normal() {
        // Its centre is 0.1 m *below* the ground at sea level (z = +0.1):
        // its bottom is 0.5 m deep.
        let contact = contact(
            SPHERE,
            DVec3::new(3.0, 4.0, 0.1),
            Collider::HalfSpace,
            DVec3::ZERO,
        )
        .expect("they overlap");
        assert_eq!(contact.normal, DVec3::NEG_Z);
        assert!((contact.depth - (RADIUS + 0.1)).abs() < PUSH_TOLERANCE);
    }

    #[test]
    fn a_ball_in_the_air_does_not_touch_the_ground() {
        assert_eq!(
            contact(SPHERE, thrown().position, Collider::HalfSpace, DVec3::ZERO),
            None
        );
    }

    #[test]
    fn the_ground_surface_passes_through_its_body_position() {
        // Ground 10 m up (z = -10); a ball centred 9.5 m up has its bottom
        // at 9.1 m: 0.9 m deep.
        let surface = DVec3::new(50.0, 50.0, -10.0); // only its height matters
        let contact = contact(
            SPHERE,
            DVec3::new(0.0, 0.0, -9.5),
            Collider::HalfSpace,
            surface,
        )
        .expect("they overlap");
        assert!((contact.depth - 0.9).abs() < PUSH_TOLERANCE);
    }

    #[test]
    fn overlapping_spheres_are_pushed_apart_along_the_line_between_centres() {
        // 1 m apart east-west, radii 0.4 + 1: 0.4 m of overlap. The first
        // is east of the second: it must move further east.
        let contact = contact(SPHERE, DVec3::new(0.0, 1.0, 0.0), BIG_SPHERE, DVec3::ZERO)
            .expect("they overlap");
        assert_eq!(contact.normal, DVec3::Y);
        assert!((contact.depth - 0.4).abs() < PUSH_TOLERANCE);
    }

    #[test]
    fn spheres_apart_do_not_touch() {
        assert_eq!(
            contact(SPHERE, DVec3::new(0.0, 2.0, 0.0), BIG_SPHERE, DVec3::ZERO),
            None
        );
    }

    #[test]
    fn spheres_with_the_same_centre_are_pushed_apart_upwards() {
        // No line between the centres: any direction would do.
        let contact = contact(SPHERE, DVec3::ZERO, BIG_SPHERE, DVec3::ZERO).expect("they overlap");
        assert_eq!(contact.normal, DVec3::NEG_Z);
    }

    #[test]
    fn detection_works_either_way_round() {
        let ball = DVec3::new(0.0, 0.0, 0.1);
        let ball_vs_ground = contact(SPHERE, ball, Collider::HalfSpace, DVec3::ZERO);
        let ground_vs_ball = contact(Collider::HalfSpace, DVec3::ZERO, SPHERE, ball);
        assert_eq!(ground_vs_ball, ball_vs_ground.map(Contact::flipped));
        // Two endless planes never meet in a world: both would be static.
        let plane = Collider::HalfSpace;
        assert_eq!(contact(plane, DVec3::ZERO, plane, DVec3::ZERO), None);
    }

    /// Velocities after `bounce` between `a` and `b` (velocity, mass;
    /// `None` for a static body), along `normal`, with `restitution`.
    fn after_bounce(
        a: (DVec3, Option<f64>),
        b: (DVec3, Option<f64>),
        normal: DVec3,
        restitution: f64,
    ) -> (DVec3, DVec3) {
        let inverse = |mass: Option<f64>| mass.map_or(0.0, |m| 1.0 / m);
        let (mut velocity_a, mut velocity_b) = (a.0, b.0);
        bounce(
            (&mut velocity_a, inverse(a.1)),
            (&mut velocity_b, inverse(b.1)),
            normal,
            restitution,
        );
        (velocity_a, velocity_b)
    }

    /// Velocity tolerance: an impulse is a few products and a division.
    const IMPULSE_TOLERANCE: f64 = 1e-12;

    fn assert_close(actual: DVec3, expected: DVec3) {
        let gap = (actual - expected).length();
        assert!(gap < IMPULSE_TOLERANCE, "{actual} instead of {expected}");
    }

    #[test]
    fn against_the_ground_falling_stops_and_sliding_goes_on() {
        // A falling ball on a static ground, dead (restitution 0): its
        // downward speed is cancelled; no friction yet, so it keeps sliding.
        let falling = DVec3::new(5.0, 6.0, 2.0);
        let (ball, _) = after_bounce((falling, Some(1.0)), (DVec3::ZERO, None), DVec3::NEG_Z, 0.0);
        assert_eq!(ball, DVec3::new(5.0, 6.0, 0.0));
    }

    #[test]
    fn separating_bodies_are_left_alone() {
        let rising = DVec3::new(1.0, 0.0, -3.0);
        let (ball, _) = after_bounce((rising, Some(1.0)), (DVec3::ZERO, None), DVec3::NEG_Z, 0.0);
        assert_eq!(ball, rising);
    }

    #[test]
    fn equal_balls_meeting_elastically_exchange_velocities() {
        // Head-on, along x: `a` (on the left) must move left (-x) to get
        // out of `b`. Elastic: like billiard balls, or Newton's cradle.
        let (a, b) = after_bounce(
            (DVec3::new(3.0, 0.0, 0.0), Some(2.0)),
            (DVec3::new(-1.0, 0.0, 0.0), Some(2.0)),
            DVec3::NEG_X,
            1.0,
        );
        assert_close(a, DVec3::new(-1.0, 0.0, 0.0));
        assert_close(b, DVec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn unequal_masses_follow_the_textbook_elastic_collision() {
        // 1 kg at 2 m/s hits 3 kg at rest. In 1D, elastic:
        // v1' = (m1 - m2) / (m1 + m2) v = -1 m/s (bounces back),
        // v2' = 2 m1 / (m1 + m2) v = +1 m/s.
        let (light, heavy) = after_bounce(
            (DVec3::new(2.0, 0.0, 0.0), Some(1.0)),
            (DVec3::ZERO, Some(3.0)),
            DVec3::NEG_X,
            1.0,
        );
        assert_close(light, DVec3::new(-1.0, 0.0, 0.0));
        assert_close(heavy, DVec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn momentum_is_conserved_and_restitution_scales_the_separation() {
        // Any restitution: momentum before = after. Separation speed =
        // restitution × approach speed (here 4 m/s along the normal).
        for restitution in [0.0, 0.5, 1.0] {
            let (va, vb) = (DVec3::new(3.0, 1.0, 0.0), DVec3::new(-1.0, 2.0, 0.0));
            let (ma, mb) = (2.0, 5.0);
            let (a, b) = after_bounce((va, Some(ma)), (vb, Some(mb)), DVec3::NEG_X, restitution);
            assert_close(a * ma + b * mb, va * ma + vb * mb);
            let separation = (a - b).dot(DVec3::NEG_X);
            assert!((separation - restitution * 4.0).abs() < IMPULSE_TOLERANCE);
            // Across the normal (y), nothing changes: no friction.
            assert_eq!((a.y, b.y), (va.y, vb.y));
        }
    }

    #[test]
    fn overlapping_bodies_are_pushed_apart_by_inverse_mass() {
        let contact = Contact {
            normal: DVec3::NEG_X,
            depth: 0.3,
        };
        let separated = |mass_a: Option<f64>, mass_b: Option<f64>| {
            let inverse = |mass: Option<f64>| mass.map_or(0.0, |m| 1.0 / m);
            let (mut a, mut b) = (DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
            separate(
                (&mut a, inverse(mass_a)),
                (&mut b, inverse(mass_b)),
                contact,
            );
            (a.x, b.x)
        };
        // Equal: each moves half the overlap. Against a static body: the
        // moving one does it all. Two static bodies: nothing moves.
        let (a, b) = separated(Some(1.0), Some(1.0));
        assert!((a + 0.15).abs() < IMPULSE_TOLERANCE && (b - 1.15).abs() < IMPULSE_TOLERANCE);
        assert_eq!(separated(Some(1.0), None), (-0.3, 1.0));
        assert_eq!(separated(None, None), (0.0, 1.0));
    }

    #[test]
    fn two_static_bodies_never_bounce() {
        let still = DVec3::new(1.0, 0.0, 0.0);
        let (a, b) = after_bounce((still, None), (-still, None), DVec3::NEG_X, 1.0);
        assert_eq!((a, b), (still, -still));
    }

    #[test]
    fn the_smaller_restitution_of_the_two_counts() {
        let bouncy = Body {
            restitution: 0.9,
            ..thrown_ball()
        };
        let dead = Body {
            restitution: 0.0,
            ..ground(0.0)
        };
        assert_eq!(restitution(&bouncy, &dead), 0.0);
        assert_eq!(restitution(&bouncy, &bouncy), 0.9);
    }

    #[test]
    fn without_a_ground_bodies_fall_forever() {
        // The ground is not built in: it's a static body the world may or
        // may not have (like a Rapier fixed body or a Godot StaticBody3D),
        // placed by the scene.
        let mut world = World::new(STANDARD_GRAVITY);
        let ball = add(&mut world, thrown_ball());
        for _ in 0..600 {
            world.step(DT);
        }
        let z = get(&world, ball).motion.position.z;
        assert!(z > 100.0, "fell through: no ground");
    }

    #[test]
    fn a_raised_ground_holds_bodies_at_its_elevation() {
        let mut world = World::new(STANDARD_GRAVITY);
        add(&mut world, ground(10.0));
        let ball = add(&mut world, thrown_ball());
        for _ in 0..600 {
            world.step(DT);
        }
        let altitude = -get(&world, ball).motion.position.z;
        assert_eq!(altitude, 10.0 + RADIUS);
    }

    #[test]
    fn a_static_sphere_is_an_obstacle() {
        // No gravity: a ball slides east at 2 m/s into a static sphere of
        // 1 m radius, 5 m east. It stops touching it (centres 1.4 m apart),
        // its eastward speed cancelled.
        let mut world = World::new(DVec3::ZERO);
        let ball = add(
            &mut world,
            Body {
                motion: Motion {
                    position: DVec3::ZERO,
                    velocity: DVec3::new(0.0, 2.0, 0.0),
                },
                ..thrown_ball()
            },
        );
        let rock_body = Body {
            body_type: BodyType::Static,
            motion: Motion {
                position: DVec3::new(0.0, 5.0, 0.0),
                velocity: DVec3::ZERO,
            },
            collider: BIG_SPHERE,
            restitution: 0.0,
        };
        let rock = add(&mut world, rock_body);
        for _ in 0..(5 * 120) {
            world.step(DT);
        }
        let ball = get(&world, ball).motion;
        assert!((ball.position.y - (5.0 - 1.0 - RADIUS)).abs() < PUSH_TOLERANCE);
        assert!(ball.velocity.y.abs() < PUSH_TOLERANCE);
        assert_eq!(get(&world, rock), rock_body);
    }

    #[test]
    fn a_moving_half_space_is_rejected() {
        let mut falling_ground = ground(0.0);
        falling_ground.body_type = BodyType::Dynamic { mass: 1.0 };
        let mut world = World::new(STANDARD_GRAVITY);
        let error = world.insert(falling_ground);
        assert_eq!(error, Err(WorldError::MustBeStatic));
        assert_eq!(
            error.unwrap_err().to_string(),
            "a dynamic body's collider can only be static"
        );
        assert_eq!(world, World::new(STANDARD_GRAVITY), "not added");
    }

    #[test]
    fn a_moving_body_needs_a_positive_mass() {
        for mass in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let weightless = Body {
                body_type: BodyType::Dynamic { mass },
                ..thrown_ball()
            };
            let error = World::new(STANDARD_GRAVITY).insert(weightless);
            assert_eq!(error, Err(WorldError::InvalidMass), "{mass}");
        }
        assert_eq!(
            WorldError::InvalidMass.to_string(),
            "a dynamic body's mass isn't positive"
        );
    }

    #[test]
    fn a_body_of_another_world_is_not_found_nor_pushed() {
        let mut big = World::new(DVec3::ZERO);
        add(&mut big, thrown_ball());
        let second = add(&mut big, thrown_ball());
        let mut small = World::new(DVec3::ZERO);
        let only = add(&mut small, floating(1.0));
        assert_eq!(small.body(second), None);
        small.add_force(second, DVec3::new(1.0, 0.0, 0.0)); // ignored
        small.step(DT);
        assert_eq!(get(&small, only), floating(1.0));
    }

    /// A ball of `mass` kg at rest at the origin, with no gravity.
    fn floating(mass: f64) -> Body {
        Body {
            body_type: BodyType::Dynamic { mass },
            motion: Motion {
                position: DVec3::ZERO,
                velocity: DVec3::ZERO,
            },
            collider: SPHERE,
            restitution: 0.0,
        }
    }

    #[test]
    fn a_constant_force_gives_a_constant_acceleration_of_force_over_mass() {
        // 2 N on 4 kg: 0.5 m/s² north. Exact solution: v = a t,
        // x = a t²/2. Semi-implicit Euler gets v exactly, and x with its
        // known first-order error, a·t·dt/2 (see the free-fall test).
        let mut world = World::new(DVec3::ZERO);
        let ball = add(&mut world, floating(4.0));
        for _ in 0..120 {
            world.add_force(ball, DVec3::new(2.0, 0.0, 0.0)); // every step
            world.step(DT);
        }
        let (a, t) = (0.5, 1.0);
        let motion = get(&world, ball).motion;
        assert!((motion.velocity.x - a * t).abs() < 1e-12);
        let euler_error = 0.5 * a * t * DT;
        assert!((motion.position.x - (0.5 * a * t * t + euler_error)).abs() < 1e-12);
    }

    #[test]
    fn the_same_force_moves_a_heavier_body_less() {
        let mut world = World::new(DVec3::ZERO);
        let light = add(&mut world, floating(1.0));
        let heavy = add(&mut world, floating(2.0));
        for body in [light, heavy] {
            world.add_force(body, DVec3::new(0.0, 3.0, 0.0));
        }
        world.step(DT);
        let speed = |body| get(&world, body).motion.velocity.y;
        assert!((speed(light) - 2.0 * speed(heavy)).abs() < 1e-12);
    }

    #[test]
    fn a_force_acts_only_during_the_step_it_is_given_for() {
        // Pushed for one step, then coasting: its speed stays what that
        // one push gave it (no gravity, nothing else).
        let mut world = World::new(DVec3::ZERO);
        let ball = add(&mut world, floating(1.0));
        world.add_force(ball, DVec3::new(10.0, 0.0, 0.0));
        world.step(DT);
        let after_push = get(&world, ball).motion.velocity;
        assert_eq!(after_push, DVec3::new(10.0 * DT, 0.0, 0.0));
        for _ in 0..10 {
            world.step(DT);
        }
        assert_eq!(get(&world, ball).motion.velocity, after_push);
    }

    #[test]
    fn forces_add_up() {
        let mut world = World::new(DVec3::ZERO);
        let ball = add(&mut world, floating(1.0));
        world.add_force(ball, DVec3::new(32.0, 0.0, 0.0));
        world.add_force(ball, DVec3::new(32.0, 0.0, 0.0));
        world.step(1.0 / 64.0);
        assert_eq!(get(&world, ball).motion.velocity.x, 1.0); // 64 N for 1/64 s
    }

    #[test]
    fn forces_do_not_move_static_bodies() {
        let mut world = World::new(STANDARD_GRAVITY);
        let floor = add(&mut world, ground(0.0));
        world.add_force(floor, DVec3::new(1e6, 1e6, 1e6));
        world.step(DT);
        assert_eq!(get(&world, floor), ground(0.0));
    }

    #[test]
    fn only_half_spaces_must_be_static() {
        assert!(Collider::HalfSpace.must_be_static());
        assert!(!SPHERE.must_be_static());
    }

    #[test]
    fn static_bodies_never_move() {
        // Not even with a velocity, under gravity: static means static.
        let mut moving_ground = ground(0.0);
        moving_ground.motion.velocity = DVec3::new(1.0, 2.0, 3.0);
        let mut world = World::new(STANDARD_GRAVITY);
        let floor = add(&mut world, moving_ground);
        for _ in 0..120 {
            world.step(DT);
        }
        assert_eq!(get(&world, floor), moving_ground);
    }

    #[test]
    fn every_static_ground_applies() {
        // Two grounds: the higher one wins, as the ball lands on it first.
        let mut world = World::new(STANDARD_GRAVITY);
        add(&mut world, ground(0.0));
        let ball = add(&mut world, thrown_ball());
        add(&mut world, ground(5.0));
        for _ in 0..600 {
            world.step(DT);
        }
        assert_eq!(-get(&world, ball).motion.position.z, 5.0 + RADIUS);
    }

    #[test]
    fn gravity_is_a_setting_of_the_world() {
        // Rapier and Box2D make gravity a world parameter too: the engine
        // assumes no planet. Without gravity, a thrown ball flies straight.
        let mut world = World::new(DVec3::ZERO);
        let ball = add(&mut world, thrown_ball());
        add(&mut world, ground(0.0));
        for _ in 0..120 {
            world.step(DT);
        }
        assert_eq!(get(&world, ball).motion.velocity, DVec3::new(5.0, 0.0, 0.0));
        let expected = DVec3::new(5.0, 0.0, -20.0);
        assert!((get(&world, ball).motion.position - expected).length() < 1e-9);

        // Gravity can point anywhere: westwards, it bends the path west.
        world.gravity = DVec3::new(0.0, -2.0, 0.0);
        world.step(1.0);
        assert_eq!(
            get(&world, ball).motion.velocity,
            DVec3::new(5.0, -2.0, 0.0)
        );
    }

    #[test]
    fn a_world_starts_at_time_zero_and_finds_each_body_by_its_id() {
        let mut world = World::new(STANDARD_GRAVITY);
        let floor = add(&mut world, ground(0.0));
        let thrown = add(&mut world, thrown_ball());
        assert_eq!(world.time(), 0.0);
        assert_ne!(floor, thrown);
        assert_eq!(get(&world, floor), ground(0.0));
        assert_eq!(get(&world, thrown), thrown_ball());
    }

    #[test]
    fn ball_lands_at_the_free_fall_time_and_stays_on_the_ground() {
        // Its centre falls h = 20 m - radius, which takes t = sqrt(2h/g).
        let impact = (2.0 * (20.0 - RADIUS) / G).sqrt();
        let mut world = World::new(STANDARD_GRAVITY);
        let ball = add(&mut world, thrown_ball());
        add(&mut world, ground(0.0));
        let altitude = |world: &World| -get(world, ball).motion.position.z;

        while world.time() < impact - 2.0 * DT {
            world.step(DT);
        }
        assert!(altitude(&world) > RADIUS, "landed too early");

        while world.time() < impact + 2.0 * DT {
            world.step(DT);
        }
        assert_eq!(altitude(&world), RADIUS, "not landed in time");
        assert_eq!(get(&world, ball).motion.velocity.z, 0.0);

        for _ in 0..600 {
            world.step(DT); // 5 more seconds: it must not sink or bounce
        }
        assert_eq!(altitude(&world), RADIUS);
    }

    #[test]
    fn every_body_is_simulated_with_its_own_collider() {
        let mut world = World::new(STANDARD_GRAVITY);
        let thrown = add(&mut world, thrown_ball());
        let resting = add(&mut world, resting_ball(5.0, 1.0));
        add(&mut world, ground(0.0));
        for _ in 0..120 {
            world.step(DT);
        }
        // The thrown ball fell; the big one, resting on its 1 m radius,
        // stayed put. (They pass through each other: only static grounds
        // stop bodies so far.)
        assert!(get(&world, thrown).motion.position.z > -20.0);
        assert_eq!(get(&world, resting), resting_ball(5.0, 1.0));
    }

    /// A ball of `radius` 0.4 m and 1 kg at `north` metres, moving north
    /// at `speed`, with `restitution`, in weightless space.
    fn billiard_ball(north: f64, speed: f64, restitution: f64) -> Body {
        Body {
            body_type: BodyType::Dynamic { mass: 1.0 },
            motion: Motion {
                position: DVec3::new(north, 0.0, 0.0),
                velocity: DVec3::new(speed, 0.0, 0.0),
            },
            collider: SPHERE,
            restitution,
        }
    }

    #[test]
    fn a_chain_reaction_passes_the_momentum_down_a_row_of_balls() {
        // Newton's cradle: five touching elastic balls, a sixth comes in
        // at 1 m/s. The last one should leave at about 1 m/s and the others
        // end about still, with momentum (5 kg × 0 + 1 kg × 1 m/s) kept.
        let mut world = World::new(DVec3::ZERO);
        let striker = add(&mut world, billiard_ball(-1.0, 1.0, 1.0));
        let row: Vec<BodyId> = (0..5)
            .map(|i| add(&mut world, billiard_ball(0.8 * f64::from(i), 0.0, 1.0)))
            .collect();
        for _ in 0..240 {
            world.step(DT);
        }
        let speed = |id| get(&world, id).motion.velocity.x;
        let momentum = speed(striker) + row.iter().map(|&id| speed(id)).sum::<f64>();
        assert!((momentum - 1.0).abs() < 1e-9, "momentum {momentum}");
        assert!(
            (speed(row[4]) - 1.0).abs() < 0.05,
            "last ball: {}",
            speed(row[4])
        );
        assert!(speed(striker).abs() < 0.05, "striker: {}", speed(striker));
    }

    #[test]
    fn a_bouncy_ball_on_a_dead_ground_still_lands_dead() {
        // The ground's restitution (0) is the smaller: no bounce.
        let mut world = World::new(STANDARD_GRAVITY);
        let ball = add(
            &mut world,
            Body {
                restitution: 0.9,
                ..thrown_ball()
            },
        );
        add(&mut world, ground(0.0));
        for _ in 0..600 {
            world.step(DT);
        }
        assert_eq!(-get(&world, ball).motion.position.z, RADIUS);
        assert_eq!(get(&world, ball).motion.velocity.z, 0.0);
    }

    #[test]
    fn touching_balls_resting_on_the_ground_stay_put() {
        let mut world = World::new(STANDARD_GRAVITY);
        add(&mut world, ground(0.0));
        let left = Body {
            restitution: 0.9,
            ..resting_ball(0.0, RADIUS)
        };
        let right = Body {
            motion: Motion {
                position: DVec3::new(2.0 * RADIUS, 0.0, -RADIUS),
                velocity: DVec3::ZERO,
            },
            ..left
        };
        let (a, b) = (add(&mut world, left), add(&mut world, right));
        for _ in 0..600 {
            world.step(DT);
        }
        let near =
            |id, body: Body| (get(&world, id).motion.position - body.motion.position).length();
        assert!(near(a, left) < 1e-9 && near(b, right) < 1e-9);
    }

    #[test]
    fn restitution_must_be_between_0_and_1() {
        for restitution in [-0.1, 1.5, f64::NAN] {
            let body = Body {
                restitution,
                ..thrown_ball()
            };
            let error = World::new(STANDARD_GRAVITY).insert(body);
            assert_eq!(error, Err(WorldError::InvalidRestitution), "{restitution}");
        }
        assert_eq!(
            WorldError::InvalidRestitution.to_string(),
            "a body's restitution isn't within 0..=1"
        );
    }

    #[test]
    fn simulation_is_deterministic() {
        // Two worlds fed the same steps must be bit-identical (assert_eq!,
        // not a tolerance). Replays in step 10 rely on this.
        let build = || {
            let mut world = World::new(STANDARD_GRAVITY);
            // Colliding balls, bouncing: the most order-sensitive case.
            let bodies = [
                ground(0.0),
                thrown_ball(),
                resting_ball(5.0, 1.0),
                billiard_ball(-1.0, 3.0, 0.8),
                billiard_ball(0.0, 0.0, 0.8),
            ];
            for body in bodies {
                add(&mut world, body);
            }
            world
        };
        let (mut a, mut b) = (build(), build());
        for _ in 0..1000 {
            a.step(DT);
            b.step(DT);
        }
        assert_eq!(a, b);
    }
}
