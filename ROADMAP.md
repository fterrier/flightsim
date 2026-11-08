# Roadmap

Small steps. Each step is runnable, has tests, and is tagged `step-NN` in
git, so you can read one step at a time:

```sh
git diff step-00 step-01        # what step 1 changed
git checkout step-01            # the code as it was after step 1
```

## Steps

| | Step | You'll see | Concepts |
|---|---|---|---|
| ✅ | **0. Harness** | `cargo test` green locally and in CI | Cargo workspace, pinned toolchain, CI (fmt, clippy, test, ≥ 95% coverage), TDD workflow (AGENTS.md) |
| ⬜ | **1. Window + game loop** | A dot moving at constant speed; FPS and sim-time overlay; keys to simulate slow frames | Fixed-timestep accumulator, update vs render, spiral of death |
| ⬜ | **2. Falling ball** | A ball drops and stops on the ground | Numerical integration (semi-implicit Euler), testing against an analytic solution |
| ⬜ | **3. 2D point-mass plane** (side view) | Throttle and pitch keys; the plane rolls, lifts off, climbs, stalls | Lift, drag, thrust, weight, air density, angle of attack |
| ⬜ | **4. 2D plane that rotates** | Pitch is now a dynamic state driven by the elevator; the plane settles at its own trim attitude | Moments, moment of inertia, static stability (the tail), trim |
| ⬜ | **5. 3D: 6 degrees of freedom** | Box plane over a grid, chase camera | Body vs world frames, quaternions, inertia tensor |
| ⬜ | **6. Data-driven Cessna 152** | Aircraft defined in `aircraft/c152.toml`; tests compare stall speed, cruise speed and climb rate to the POH | Aerodynamic coefficient build-up, model validation |
| ⬜ | **7. Smooth rendering + replay** | Interpolated rendering; record a flight and replay it identically | State interpolation, determinism |
| ⬜ | **8. Many entities (ECS)** | A second aircraft and some buildings in the world | Entity-Component-System, composition over inheritance (see decision below) |
| ⬜ | **9. Ground and collisions** | Real takeoff roll and landing on gear; buildings are solid | Spring-damper contacts, friction, collision shapes |

### Later (unordered)

- Engine (Lycoming O-235) and fixed-pitch propeller model
- Wind, gusts, turbulence (Dryden model)
- Our own GPU renderer with `wgpu` (meshes, camera, lighting, glTF models)
- Instruments / HUD (`egui`)
- Gamepad and joystick input
- A second flight model: blade element (geometry-based, like X-Plane)
- Round earth (WGS84), real terrain elevation
- More aircraft

## Architecture

```
crates/
  physics/   pure simulation: state structs + functions that update them
  app/       window, input, game loop (owns the clock), drawing
```

Rules:

1. **Physics never knows about time sources, input devices or graphics.**
   It exposes plain data and functions like `step(&mut state, &controls, dt)`.
2. **The app owns the loop.** It reads the clock, polls input, calls physics
   a whole number of fixed steps, then draws.
3. **Rendering only reads** simulation state, never writes it.
4. **New crates only when a boundary hurts.** Likely future splits:
   `aircraft` (data loading), `collision`, `render` (wgpu).

## Decisions

### Game loop: fixed timestep

Physics advances in fixed steps (e.g. 1/120 s) regardless of frame rate;
rendering happens once per frame. Real frame time is accumulated and
consumed in whole steps; frame time is capped so a stall (debugger, window
drag) doesn't trigger an endless catch-up ("spiral of death"). Later
(step 7) rendering interpolates between the last two physics states.
This makes the simulation deterministic: same inputs, same flight.
Reference: Glenn Fiedler, *Fix Your Timestep!*

### Graphics: macroquad now, wgpu later

[macroquad](https://github.com/not-fl3/macroquad) is a small immediate-mode
library (raylib-like): draw a line, draw a cube, done. It compiles fast and
does enough 2D and basic 3D for the physics steps. Writing our own `wgpu`
renderer is a later learning milestone of its own. Full engines (Bevy,
Fyrox) were rejected: they own the game loop and the world, which is what we
want to learn to build.

### Flight model: coefficient-based first

Two families exist: *coefficient-based* (JSBSim, most research/training
sims; forces from tables like CL(α) measured in wind tunnels / flight tests)
and *geometry-based* (X-Plane's blade element, MSFS's surface elements; the
aircraft is cut into small surfaces and forces summed). We start
coefficient-based: simpler, and directly checkable against published C152
data. Blade element can come later as a second model.

### Rigid-body physics: our own, not Rapier/Avian

General physics engines solve many-body contact problems. Flight dynamics
is dominated by aerodynamic forces on one body with few contacts. The 6DOF
equations are ~100 lines and the core thing to learn. A collision library
(e.g. `parry`) may be used later for buildings.

### Entities: ECS-shaped now, real ECS at step 8

We want multiple aircraft, buildings, maybe ground vehicles. These share
*some* aspects but not all:

| | position | rigid body | aero | engine | mesh | collider |
|---|---|---|---|---|---|---|
| aircraft | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| building | ✓ | | | | ✓ | ✓ |
| car | ✓ | ✓ | | ✓ | ✓ | ✓ |

The old C++ code used inheritance (`Plane : BasicObject`), which breaks down
quickly with such mixes. The modern answer is **composition**, and its
standard form is an **Entity-Component-System**: entities are IDs,
components are plain data attached to them, systems are functions that run
over every entity having a given set of components (e.g. "draw everything
with position + mesh").

But with one entity, an ECS library is pure overhead and makes the code
harder to read. So:

- **Steps 1-7: ECS-shaped code without an ECS library.** State is split into
  small plain-data structs (future components: `Body`, `Aero`, ...) and
  behaviour lives in free functions taking only the components they need
  (future systems). A `World` struct holds everything.
- **Physics stays ECS-agnostic forever.** Physics functions take components
  (`&mut Body`), never a `World` or an ECS query. Only the app/world layer
  knows how entities are stored. This keeps the choice reversible.
- **Step 8: introduce a real ECS** when the second kind of entity arrives.
  Options to decide then: write a minimal one ourselves (~200 lines, great
  for learning how they work), [hecs](https://github.com/Ralith/hecs)
  (minimal library, we keep owning the loop), or `bevy_ecs` standalone
  (powerful, heavier).

## References

- Beard & McLain, *Small Unmanned Aircraft: Theory and Practice*: most
  accessible 6DOF derivation
- Stevens & Lewis, *Aircraft Control and Simulation*: the flight-simulation
  reference (trim, linearization)
- [JSBSim](https://github.com/JSBSim-Team/jsbsim) and its `aircraft/c152`:
  a real flight dynamics model to compare against
- Glenn Fiedler, [Fix Your Timestep!](https://gafferongames.com/post/fix_your_timestep/)
- Robert Nystrom, [Game Programming Patterns](https://gameprogrammingpatterns.com/)
  (Game Loop, Component chapters)
- [Learn Wgpu](https://sotrh.github.io/learn-wgpu/) for the later renderer
