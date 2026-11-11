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
| ✅ | **3. Smooth 2D camera** | Motion stays smooth at any display rate; the camera glides after the ball, follows it vertically, zoom is limited; a key switches follow / fixed camera | Interpolation between physics states, frame-rate-independent smoothing, dead zones, camera as a tested module |
| ✅ | **3b. Engine / game split** | Same picture; the code is now a reusable engine plus a game built on it | Engine vs game (and the dependency direction), `Game` trait, engine-owned input and the Command pattern, humble platform layer, separate real/fixed clocks |
| ✅ | **4. Ball in 3D** | A ball thrown in any direction over a ground grid, lit in 3D | `DVec3`, axis conventions: NED physics vs y-up graphics, converted in one place in the renderer (see decision below) |
| ⬜ | **5. 3D cameras** | Chase camera (smoothed, behind the target) and orbit camera (mouse), switched with a key | 3D camera maths: look-at, spherical coordinates, smoothing in 3D |
| ⬜ | **6. Point-mass plane** | Throttle and pitch keys; the plane rolls, lifts off, climbs, stalls (moving in the vertical plane) | Mass, lift, drag, thrust, weight, air density, angle of attack |
| ⬜ | **7. Plane that pitches** | Pitch is now a dynamic state driven by the elevator; the plane settles at its own trim attitude | Moments, moment of inertia, static stability (the tail), trim |
| ⬜ | **8. Full 6 degrees of freedom** | Roll and yaw: the plane banks and turns | Body vs world frames, quaternions, inertia tensor |
| ⬜ | **9. Data-driven Cessna 152** | Aircraft defined in `aircraft/c152.toml`; tests compare stall speed, cruise speed and climb rate to the POH | Aerodynamic coefficient build-up, model validation |
| ⬜ | **10. Replay** | Record a flight and replay it identically | Determinism, input recording |
| ⬜ | **11. Many entities (ECS)** | A second aircraft and some buildings in the world; a benchmark of the physics → scene sync | Entity-Component-System, composition over inheritance (see decision below), data-oriented design: cache-friendly contiguous layout (SoA / archetypes), instanced rendering, benchmarking (`criterion`) |
| ⬜ | **12. Ground and collisions** | Real takeoff roll and landing on gear; buildings are solid | Spring-damper contacts, friction, collision shapes |

The order was revised after step 2: cameras and 3D come before the plane,
so the plane is built directly in 3D (no throwaway 2D plane), kiss3d's 3D
side is proven on a trivial object, and interpolation moved next to the
camera work, where its absence would show as judder. The plane still
starts with motion in the vertical plane only (steps 6-7), deferring
rotation maths (quaternions) to step 8.

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

The code is split like every game engine: a reusable **engine**, and the
**game** built on it. The game depends on the engine, never the reverse,
and the crate boundaries make the compiler enforce it.

```
crates/
  physics/        ENGINE  physics: bodies, colliders, integration      (pure)
  engine/         ENGINE  core, no graphics library, fully tested:
                    engine.rs       Engine: runs a frame; the `Game` trait
                    simulation.rs   physics world + fixed steps + interpolation
                    timestep.rs     the fixed-timestep clock
                    camera.rs       the camera: smooth follow, zoom, what's visible
                    render.rs       Renderer (proxies, debug drawing) against
                                    the RenderBackend interface
                    hud.rs          text overlay; FrameStats (FPS, real time)
                    input.rs        Key, InputEvent, Command
                    scene.rs        the scene format (objects = body + visual)
  engine_kiss3d/  ENGINE  platform layer: window, run loop, input
                          translation, RenderBackend with kiss3d   (humble)
  flightsim/      GAME    what game it is: scenarios, key bindings, HUD
                          lines; main() = engine_kiss3d::run(FlightSim)
```

A game plugs in through `engine::Game`: its physics rate, its scene, its
input bindings (input event -> `Command`) and its own HUD lines, written
from a `WorldView`: the world's data, borrowed read-only (no copy), without
the machinery. Stepping physics is the engine's job: `World::step` is
public in the `physics` library (as `b2World_Step` in Box2D or
`PhysicsPipeline::step` in Rapier, for the engine that drives it) but not
part of the game-facing API. Everything else is the engine's. Games
depend on `engine` alone, a facade like the `bevy` crate: it re-exports
the physics content types, and its public API is a short, curated list
in `engine/src/lib.rs`. What only platform crates need (`Engine`,
`RenderBackend`...) lives apart in `engine::platform`: Rust has no
"friend" visibility between crates, so it must be `pub`, but games never
import it (checked in CI).

Rules:

1. **Physics never knows about time sources, input devices or graphics.**
   It exposes plain data and functions like `step(&mut state, &controls, dt)`.
2. **The engine owns the loop**, not the game. The platform layer reads
   the clock and input; `Engine` runs a whole number of fixed physics
   steps, then draws. The game only supplies content, bindings and HUD.
3. **Rendering only reads** simulation state, never writes it. All logic
   lives in tested code; the `engine_kiss3d` crate and the game's
   `main.rs` are humble shells around it (see AGENTS.md "Coverage").
4. **New crates only when a boundary hurts.** The engine/game and
   core/platform boundaries did (step 3b). Likely future splits:
   `aircraft` (data loading), `collision`.
5. **Physics → scene in one place.** Graphics keep *render proxies*
   (scene nodes) for simulated objects; once per frame, one place copies
   the (interpolated) physics state into them: `Renderer::draw`, in
   `engine/src/render.rs`.
6. **Content is data.** A scene (a "level", defined by the game) lists
   everything that exists as objects, *the ground included*: as in
   engines (Godot's `StaticBody3D`, Avian's `RigidBody::Static`), the
   ground is a static object, not an engine built-in. Each object is
   described whole: its body, dynamic (motion + collider) or static (a
   collider that never moves), for the simulation, and its `Visual`
   (shape + colour, later a glTF mesh + material) for the renderer. Only
   what exists nowhere in particular (background, gravity) is a scene
   setting. Loading splits objects: dynamic ones become physics bodies,
   static ones static colliders, visuals become render proxies or grid
   lines (entity IDs replace the index bookkeeping at step 11). The game
   loop never knows what's inside, and the render interface has a few
   generic operations that never grow with content: no `draw_plane`.

## Decisions

### Game loop: fixed timestep

Physics advances in fixed steps (e.g. 1/120 s) regardless of frame rate;
rendering happens once per frame. Real frame time is accumulated and
consumed in whole steps; frame time is capped so a stall (debugger, window
drag) doesn't trigger an endless catch-up ("spiral of death"). From
step 3, rendering interpolates between the last two physics states,
storing only the previous *transform* of each moving object before each
step (`Simulation::previous`, one contiguous array), as engines do (Bevy's `PreviousPhysicalTranslation`,
Godot and Unity physics interpolation), not a copy of the whole world.
The camera follows the *interpolated* position, in real frame time
(Godot's documented approach). Cost: what you see lags the simulation by
up to one physics step.
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

### Rendering: a generic backend interface, tested renderer, humble adapter

All rendering decisions (where proxies go, f64 → f32, zoom limits, ruler
and HUD layout, later the NED → y-up conversion) live in `Renderer`,
written against `RenderBackend`: a few generic operations (create a
sphere proxy, move it, set the camera, debug lines, text). The
kiss3d implementation is a "humble object", one kiss3d call per method.

This follows established practice: engines put the graphics API behind
an interface with generic operations (Unreal's RHI and scene proxies,
Godot's `RenderingServer` with instance handles, Bevy's render world),
and test logic against fakes of it (Unreal runs automation tests with a
"null RHI"; Meszaros' *Humble Object* pattern). Tests use a recording
fake and a null one; the null one also proves a frame allocates nothing.
Pixel-level snapshot tests of the real backend (as wgpu, Bevy and
egui_kittest do, with software GPUs on CI) can come later, once there is
3D content worth protecting.

### Axes: NED in physics, y-up in graphics (from step 4)

Flight-dynamics literature (Stevens & Lewis, Beard & McLain) uses
**NED**: x north, y east, z *down* (and body axes x forward, y right,
z down). Graphics libraries, kiss3d included, use **y up**. Physics keeps
NED so its equations match the books; the conversion happens in exactly
one place, the renderer's physics → render conversion (Architecture rule
5), as in real simulators.
Since step 4: physics in NED (altitude = -z), render axes x east, y up,
z south, converted by `to_render` in `engine/src/render.rs`.

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

### Entities: ECS-shaped now, real ECS at step 11

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

- **Steps 1-10: ECS-shaped code without an ECS library.** State is split into
  small plain-data structs (future components: `Body`, `Aero`, ...) and
  behaviour lives in free functions taking only the components they need
  (future systems). A `World` struct holds everything.
- **Physics stays ECS-agnostic forever.** Physics functions take components
  (`&mut Motion`), never a `World` or an ECS query. Only the engine layer
  knows how entities are stored. This keeps the choice reversible.
- **Step 11: introduce a real ECS** when the second kind of entity arrives.
  Options to decide then: write a minimal one ourselves (~200 lines, great
  for learning how they work), [hecs](https://github.com/Ralith/hecs)
  (minimal library, we keep owning the loop), or `bevy_ecs` standalone
  (powerful, heavier).

### Data layout: data-oriented design at step 11, measured

Copies between systems (physics → scene → GPU, see Architecture rule 5)
are normal in engines; what makes them cheap or expensive is **memory
layout**, not byte count. The CPU loads memory in 64-byte cache lines; a
cache miss to RAM costs ~100x an L1 hit. Copying 10,000 transforms is
~30 µs from a contiguous array but ~1 ms if each lives in its own heap
allocation.

One rule applies from the start, because it costs nothing in
readability: **a steady-state frame allocates no heap memory** (iterators
instead of temporary `Vec`s, text buffers reused). A test enforces it with
a counting allocator (`crates/engine/src/alloc_count.rs`): a full
`Engine` frame, simulation to HUD, allocates nothing.

The rest isn't measurable with one entity, so steps 1-10 favour
readability. At step 11, when entities multiply:

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
- Physics interpolation in engines:
  [Godot](https://docs.godotengine.org/en/stable/tutorials/physics/interpolation/physics_interpolation_introduction.html)
  (and its [camera advice](https://docs.godotengine.org/en/stable/tutorials/physics/interpolation/advanced_physics_interpolation.html)),
  [Unity](https://docs.unity3d.com/Manual/rigidbody-interpolation.html),
  [Bevy's fixed-timestep example](https://bevy.org/examples/movement/physics-in-fixed-timestep/)
- Robert Nystrom, [Game Programming Patterns](https://gameprogrammingpatterns.com/)
  (Game Loop, Component, Data Locality chapters)
- Mike Acton, [Data-Oriented Design and C++](https://www.youtube.com/watch?v=rX0ItVEVjHc)
  (CppCon 2014): the classic talk on designing around memory layout
- Richard Fabian, [Data-Oriented Design](https://www.dataorienteddesign.com/dodbook/)
  (book, free online)
- [kiss3d examples](https://github.com/dimforge/kiss3d/tree/master/examples):
  one small file per feature (glTF, instancing, shadows, text, egui...)
