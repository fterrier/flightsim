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
| ✅ | **1. Window + game loop** | A dot moving at constant speed, followed by the camera; FPS and sim-time overlay; keys to simulate slow frames | Fixed-timestep accumulator, update vs render, spiral of death, world vs screen space, camera, retained scene + render proxies |
| ✅ | **2. Falling ball** | A ball drops and stops on the ground | Numerical integration (semi-implicit Euler), testing against an analytic solution |
| ⬜ | **3. 2D point-mass plane** (side view) | Throttle and pitch keys; the plane rolls, lifts off, climbs, stalls | Lift, drag, thrust, weight, air density, angle of attack |
| ⬜ | **4. 2D plane that rotates** | Pitch is now a dynamic state driven by the elevator; the plane settles at its own trim attitude | Moments, moment of inertia, static stability (the tail), trim |
| ⬜ | **5. 3D: 6 degrees of freedom** | Box plane over a grid, chase camera | Body vs world frames, quaternions, inertia tensor |
| ⬜ | **6. Data-driven Cessna 152** | Aircraft defined in `aircraft/c152.toml`; tests compare stall speed, cruise speed and climb rate to the POH | Aerodynamic coefficient build-up, model validation |
| ⬜ | **7. Smooth rendering + replay** | Interpolated rendering; record a flight and replay it identically | State interpolation, determinism |
| ⬜ | **8. Many entities (ECS)** | A second aircraft and some buildings in the world; a benchmark of the physics → scene sync | Entity-Component-System, composition over inheritance (see decision below), data-oriented design: cache-friendly contiguous layout (SoA / archetypes), instanced rendering, benchmarking (`criterion`) |
| ⬜ | **9. Ground and collisions** | Real takeoff roll and landing on gear; buildings are solid | Spring-damper contacts, friction, collision shapes |

### Later (unordered)

- Engine (Lycoming O-235) and fixed-pitch propeller model
- Wind, gusts, turbulence (Dryden model)
- Our own 3D models (glTF from Blender), loaded with kiss3d
- Instruments / HUD (`egui`, built into kiss3d)
- Physics plots in-window (`egui_plot`)
- Floating origin (render relative to the camera) once we fly far from the origin
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
   `aircraft` (data loading), `collision`.
5. **Physics → scene in one place.** Graphics keep *render proxies*
   (scene nodes) for simulated objects; once per frame, a single sync
   function copies physics state into them (`sync_scene` in `main.rs`).

## Decisions

### Game loop: fixed timestep

Physics advances in fixed steps (e.g. 1/120 s) regardless of frame rate;
rendering happens once per frame. Real frame time is accumulated and
consumed in whole steps; frame time is capped so a stall (debugger, window
drag) doesn't trigger an endless catch-up ("spiral of death"). Later
(step 7) rendering interpolates between the last two physics states.
This makes the simulation deterministic: same inputs, same flight.
Reference: Glenn Fiedler, *Fix Your Timestep!*

### Graphics: kiss3d (on wgpu)

Requirements: we own the game loop and the world; we don't reimplement
rendering; performant (natively on Metal on macOS, where OpenGL is
deprecated and frozen at 4.1); loads our own 3D models (glTF, OBJ) later.

[kiss3d](https://github.com/dimforge/kiss3d) (0.47, rewritten on `wgpu` in
2026 by Dimforge, the makers of the Rapier physics engine, who use it for
their own physics viewers) fits all of them: explicit loop
(`while window.render_3d(..).await`), 2D and 3D scenes, glTF/OBJ loading,
instancing, PBR, shadows, text and built-in `egui`.

Rejected (Nov 2026 audit): **macroquad** (maintenance mode, open soundness
advisory, OpenGL, minimal 3D; used for the first draft of step 1),
**egui/eframe alone** (2D UI only: 3D would mean writing our own wgpu
renderer), **raw winit + wgpu** (reimplementing rendering), **three-d** and
**raylib** (OpenGL), **Bevy**, **Fyrox**, **Godot** (they own the loop and
the world: what this project is meant to teach), **rend3** (archived).
Escape hatch if we ever need city-scale scenery: Bevy or Godot; the
`physics` crate moves over unchanged.

Risk: pre-1.0, breaking releases every few months. The version is pinned;
upgrades are deliberate.

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

### Data layout: data-oriented design at step 8, measured

Copies between systems (physics → scene → GPU, see Architecture rule 5)
are normal in engines; what makes them cheap or expensive is **memory
layout**, not byte count. The CPU loads memory in 64-byte cache lines; a
cache miss to RAM costs ~100x an L1 hit. Copying 10,000 transforms is
~30 µs from a contiguous array but ~1 ms if each lives in its own heap
allocation.

With one entity none of this is measurable, so steps 1-7 favour
readability. At step 8, when entities multiply:

- **Contiguous storage**: components in dense arrays (`Vec<T>`, not
  `Vec<Box<T>>`). An archetype ECS gives this for free: each component
  type is a column, so a system walks one array (Struct of Arrays).
- **Indices, not pointers**: generational handles (index + version) to
  refer to bodies and render instances.
- **Instancing**: repeated objects (buildings) as one instance buffer, one
  upload, one draw call, instead of one scene node each.
- **Copy only what changed**: static objects are uploaded once; resting
  bodies are skipped.
- **No per-frame allocations** in hot paths: reuse buffers.
- **Measure first**: a `criterion` benchmark of the sync, comparing
  contiguous vs scattered layouts, before and after each optimisation.

## References

- Beard & McLain, *Small Unmanned Aircraft: Theory and Practice*: most
  accessible 6DOF derivation
- Stevens & Lewis, *Aircraft Control and Simulation*: the flight-simulation
  reference (trim, linearization)
- [JSBSim](https://github.com/JSBSim-Team/jsbsim) and its `aircraft/c152`:
  a real flight dynamics model to compare against
- Glenn Fiedler, [Fix Your Timestep!](https://gafferongames.com/post/fix_your_timestep/)
- Robert Nystrom, [Game Programming Patterns](https://gameprogrammingpatterns.com/)
  (Game Loop, Component, Data Locality chapters)
- Mike Acton, [Data-Oriented Design and C++](https://www.youtube.com/watch?v=rX0ItVEVjHc)
  (CppCon 2014): the classic talk on designing around memory layout
- Richard Fabian, [Data-Oriented Design](https://www.dataorienteddesign.com/dodbook/)
  (book, free online)
- [kiss3d examples](https://github.com/dimforge/kiss3d/tree/master/examples):
  one small file per feature (glTF, instancing, shadows, text, egui...)
