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
| ✅ | **5. 3D cameras** | Chase camera (smoothed, behind the target) and orbit camera (mouse), switched with a key | 3D camera maths: look-at, spherical coordinates, smoothing in 3D |
| ⬜ | **5b. Act on the world** | A ball you push with WASD (relative to the camera) and make jump with Space; it slows down when let go, jumps only from the ground, bumps into rocks. Built in increments: push, jump, drag, ground-only jump, obstacles | Mass, F = m·a, forces vs impulses, game logic per physics step (`FixedUpdate`), input per frame vs physics per step, keys held, damping, contacts reported by physics |
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

Diagrams (layers, types, one frame step by step): [ARCHITECTURE.md](ARCHITECTURE.md).

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
                    render.rs       Renderer (one proxy per object) against
                                    the RenderBackend interface
                    mesh.rs         mesh primitives: sphere, plane
                    texture.rs      Texture: images for materials (sRGB)
                    hud.rs          text overlay; FrameStats (FPS, real time)
                    input.rs        raw input -> InputEvent; Command
                    scene.rs        the scene format (objects = body + visual)
  engine_kiss3d/  ENGINE  platform layer: window, run loop, input
                          translation, RenderBackend with kiss3d   (humble)
  flightsim/      GAME    what game it is and how it's played:
                    scenarios.rs    the scenes, and what the camera follows
                    camera.rs       orbit / chase / fixed camera, its controls
                    textures.rs     the ground grid texture, drawn in code
                    game.rs         FlightSim: the `Game` (input, view, HUD)
                    main.rs         engine_kiss3d::run(FlightSim::new())
  alloc_count/    TESTS   counts heap allocations (test builds only)
```

A game plugs in through `engine::Game`: its physics rate, its scene, how
it reacts to input (its own controls and camera; engine services through
a `Command`), the `View` to draw each frame and its own HUD lines. It
reads a `WorldView`: the world's data, borrowed read-only (no copy),
including where each body is drawn, without the machinery. Stepping physics is the engine's job: `World::step` is
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
   physics engines (Rapier's fixed bodies, Jolt's static motion type,
   Godot's `StaticBody3D`), the ground is a body that never moves, not
   an engine built-in. Each object is described whole: its `Body` (type,
   motion, collider) for the simulation and its `Visual` (shape +
   colour, later a glTF mesh + material) for the renderer. Only what
   exists nowhere in particular (background, gravity) is a scene
   setting. The game *spawns* objects and adds textures, and is handed
   an ID for each (as Bevy's `spawn` and `assets.add` do). Loading
   splits objects: each entity gets a physics body and a render proxy,
   all found by IDs (see "IDs" below). The game loop never knows what's
   inside, and the render interface has a few generic operations that
   never grow with content: no `draw_plane`.

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
The blend is written once per frame into a separate *drawn* array
(`WorldView::drawn`): render state, apart from simulation state, as
Unity's interpolated `Transform` is apart from `Rigidbody.position`. The
renderer draws it, and the game's camera follows it, in real frame time
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
texture, create a mesh proxy in a colour and texture, move it, set the
camera, text). Visuals are a shape (mesh primitive) and a `Material`
(colour, optional tiled texture), as in Godot and Bevy; textures are scene
assets, uploaded once and shared. The
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

### IDs: generated, opaque, generational, typed

Everything stored and referred to elsewhere is identified by an ID from
one mechanism, the `arena` crate, as in every engine (Bevy's `Entity`,
Rapier's `RigidBodyHandle`, Jolt's `BodyID`, Godot's `RID`, `slotmap`'s
keys): **generated** by the storage on insertion, never chosen by its
user; **opaque** (no arithmetic, no forging); **generational** (index +
generation: a stale ID to something removed finds nothing, even once its
slot is reused); **typed** (an `EntityId` can't be passed for a
`TextureId`). Our own small implementation rather than `slotmap`:
generational IDs are exactly the memory-management idea this project is
here to learn.

Two levels, as bevy_rapier connects Bevy entities to Rapier handles:
games use `EntityId` and `TextureId`; each subsystem keeps its own
handles inside (`BodyId` in the physics world, `ProxyId` and
`GpuTexture` in the render backend), and the engine maps entities to
them with secondary maps. Every collection that owns things is an
`Arena`; data *about* another arena's things is a `SecondaryMap`, the
way an ECS stores a component per entity.

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
a counting allocator (the test-only `alloc_count` crate): a full
`Engine` frame, simulation to HUD, allocates nothing, and neither does
the game's per-frame code (its input handling, camera and HUD lines).

Today's layout (step 5b, IDs): one storage per subsystem, all filled in
spawn order. The scene's objects, the physics bodies (`World`), the
simulation's per-entity data (body handle, previous and drawn
positions, forces) and the renderer's proxies are separate arrays, and
entity *i*'s data sits at slot *i* in each. Lookups are array indexing,
so each loop walks a few arrays in order, which hardware prefetchers
handle well. Not one big arena of whole entities: that is the
game-object model (Unity GameObjects, Unreal Actors) that Unity DOTS and
Unreal Mass were built to move away from, since every loop would load
whole records (several cache lines) to use a few fields.

The rest isn't measurable with a few entities, so steps 1-10 favour
readability. The many-balls stress test is the benchmark scene for what
follows. At step 11, when entities multiply, split data **by how it's
accessed**, as engines do, measuring each change:

- **Hot/cold split in physics** (Box2D 3's `b2Body` / `b2BodySim` /
  32-byte `b2BodyState`; Jolt's `MotionProperties` only on moving
  bodies): integration walks a packed array of moving-body state
  (position, velocity, inverse mass, force) and nothing else. A body
  slot is two cache lines today, for about 56 bytes integration needs.
- **Group bodies by state** (Box2D's solver sets: static, awake,
  sleeping): static bodies in their own array, never walked or written
  by integration; resting bodies skipped.
- **Contiguous components, no lookup maps**: one entity store with each
  component in a packed column (Unity's 16 KiB archetype chunks, Bevy's
  tables), or physics arrays in the same order as the render proxies
  with one bulk sync per step (bevy_rapier's write-back). Today each
  entity is visited up to three times per step, in three or four arrays,
  through maps.
- **Collision pass reads only what it needs** (position and collider,
  not 80-byte bodies copied per pair), then a **broad phase** (step 12)
  so far-apart pairs are never visited.
- **Renderer walks drawn positions in proxy order**: two arrays side by
  side, no lookups.
- **Sparse data**: today's `SecondaryMap`s are indexed by slot, so they
  leave holes where an entity has no such data; fine while every entity
  has every kind of data. Once entities differ (a building has no
  motion), archetypes store only what each kind has.
- **Instancing**: repeated objects (balls, buildings) as one instance
  buffer, one upload, one draw call, instead of one scene node each.
- **No per-frame allocations** in hot paths: reuse buffers.
- **Measure first**: a `criterion` benchmark of physics and sync on the
  stress-test scene, comparing layouts before and after each change.

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
