# Architecture

How the code is organised: which layer (crate) needs which types, and how
they work together at run time. Rust has no classes: the equivalents here
are **structs and enums** (data), **traits** (interfaces, like `Game` and
`RenderBackend`), and who **owns** or **borrows** whom.

The diagrams are [Mermaid](https://mermaid.js.org/) (GitHub draws them).
They are kept up to date with the code: every public or crate-visible
type and trait must appear on this page, checked in CI
(`scripts/check-architecture.sh`, see AGENTS.md).

For the *why* behind each choice, see [ROADMAP.md](ROADMAP.md)
"Decisions".

## 1. Layers: the crates and who depends on whom

Dependencies only point one way: the game depends on the engine, the
engine never knows which game it runs. The compiler enforces it.

```mermaid
flowchart TD
    flightsim["<b>flightsim</b> (GAME)<br/>FlightSim, Camera, scenarios, textures"]
    kiss3d_crate["<b>engine_kiss3d</b> (ENGINE: platform)<br/>run, Kiss3dBackend"]
    engine["<b>engine</b> (ENGINE: core)<br/>Engine, Game, Scene, Headless, Renderer..."]
    physics["<b>physics</b> (ENGINE: pure)<br/>World, Body, BodyId, Collider"]
    arena["<b>arena</b> (ENGINE: pure)<br/>Arena, SecondaryMap, id_type!"]
    alloc["<b>alloc_count</b> (TESTS only)<br/>CountingAllocator"]
    kiss3d[("kiss3d + wgpu<br/>window, GPU")]
    glam[("glam<br/>vectors")]

    flightsim -->|"game API"| engine
    flightsim -->|"run()"| kiss3d_crate
    kiss3d_crate -->|"platform API"| engine
    kiss3d_crate --> arena
    kiss3d_crate --> kiss3d
    engine --> physics
    engine --> arena
    physics --> arena
    physics --> glam
    engine -.->|"dev-dependency"| alloc
    flightsim -.->|"dev-dependency"| alloc
    arena -.->|"dev-dependency"| alloc
```

The `engine` crate shows two faces:

| Who | Imports | Gets |
|---|---|---|
| A **game** (`flightsim`) | `engine::...` (the crate root) | what it implements (`Game`), describes (`Scene`, `SceneObject`, `Visual`, `Shape`, `Material`, `Texture`, the physics types `Body`, `Motion`...) and the IDs it's handed for it (`EntityId`, `TextureId`), reads (`WorldView`), does (`Forces`, `View`, `Command`), receives (`InputEvent`, `Key`, `MouseButton`), and a headless engine to run it in tests (`Headless`, `Flow`) |
| A **platform** (`engine_kiss3d`) | `engine::platform::...` | the engine it runs (`Engine`), the raw input it reports (`RawInput`), the interface it implements (`RenderBackend`) and what that interface carries (`Mesh`, `ProxyId`, `GpuTexture`, `Screen`, `VIEW_DISTANCE`) |

Everything else in the engine is `pub(crate)`: machinery no one outside
can touch (`Simulation`, `Renderer`, `Hud`...). CI checks the game never
imports `engine::platform`.

## 2. IDs: who owns what

Everything stored and referred to elsewhere gets an ID from the `arena`
crate, as in every engine (Bevy's `Entity`, Rapier's handles, Jolt's
`BodyID`, Godot's `RID`): **generated** by the storage on insertion,
**opaque**, **generational** (index + generation: a stale ID finds
nothing) and **typed** (`id_type!`: an `Arena` only takes its own kind of
`Id`).

```mermaid
classDiagram
    direction LR
    class Arena~K, V~ {
        insert(value) K
        get(id) Option~V~
        remove(id) Option~V~
        iter() pairs of K, V
    }
    class SecondaryMap~K, V~ {
        insert(id, value) bool
        get(id) Option~V~
        remove(id) Option~V~
        iter() pairs of K, V
    }
    class Id {
        <<trait>>
        from_raw(raw) Self
        raw() RawId
    }
    class RawId {
        index (private)
        generation (private)
    }
    Arena ..> Id : hands out
    SecondaryMap ..> Id : keyed by another arena's
    Id *-- RawId
```

The rule: **a collection that owns things is an `Arena`** and hands out
their IDs; **data about another arena's things is a `SecondaryMap`**
keyed by their IDs (as an ECS stores a component per entity). Games only
see `EntityId` and `TextureId`; each subsystem keeps its own handles
inside, and the engine maps entities to them (as bevy_rapier maps Bevy
entities to Rapier handles).

| ID | Handed out by (the `Arena`) | Data about it elsewhere (`SecondaryMap`) | Seen by |
|---|---|---|---|
| `EntityId` | `Scene::spawn` (the objects) | `Simulation`: its body and drawn position (`Interpolated`), its forces; `Renderer`: its proxy | games, engine |
| `TextureId` | `Scene::add_texture` (the images) | `Renderer::new`: its uploaded `GpuTexture` | games, engine |
| `BodyId` | `World::insert` (the bodies, like Rapier) | | physics, engine |
| `ProxyId` | the backend's `create_mesh` (its scene nodes, like Godot's `RID`) | | engine, platform |
| `GpuTexture` | the backend's `create_texture` (its GPU textures) | | engine, platform |

## 3. Data: what a scene describes, and what the physics simulates

A **scene** is a description, filled in by the game, which keeps the IDs
it's handed. Loading splits each object under its `EntityId`: its `Body`
goes to the physics `World` (which hands back a `BodyId`), its `Visual`
to the `Renderer` (a proxy).

```mermaid
classDiagram
    direction LR

    namespace physics {
        class World {
            gravity: DVec3
            bodies by BodyId
            new(gravity)
            insert(body) Result~BodyId~
            body(id) Option~Body~
            add_force(id, force)
            step(dt)
            time() f64
        }
        class WorldError {
            <<enum>>
            MustBeStatic
            InvalidMass
            InvalidRestitution
        }
        class Body {
            body_type: BodyType
            motion: Motion
            collider: Collider
            restitution: f64
        }
        class BodyType {
            <<enum>>
            Dynamic(mass)
            Static
        }
        class Motion {
            position: DVec3
            velocity: DVec3
        }
        class Collider {
            <<enum>>
            Sphere(radius)
            HalfSpace
        }
    }
    class Scene {
        objects by EntityId
        textures by TextureId
        background: Color
        gravity: DVec3
        spawn(object) EntityId
        add_texture(texture) TextureId
    }
    class SceneObject {
        body: Body
        visual: Visual
    }
    class Visual {
        shape: Shape
        material: Material
    }
    class Shape {
        <<enum>>
        Sphere(radius)
        Plane(PlaneSize)
    }
    class PlaneSize {
        <<enum>>
        Square(half_size)
        Endless
    }
    class Material {
        color: Color
        texture: Option~TiledTexture~
    }
    class TiledTexture {
        texture: TextureId
        tile_size: f64
    }
    class Texture {
        width, height
        srgba: Vec~u8~
        from_fn(w, h, pixel)
    }
    class Color {
        r, g, b, a: linear light
    }

    World *-- Body
    World ..> WorldError : insert() may fail with
    Body *-- BodyType
    Body *-- Motion
    Body *-- Collider
    Visual *-- Shape
    Visual *-- Material
    Shape *-- PlaneSize
    Material *-- Color
    Material o-- TiledTexture
    TiledTexture ..> Texture : refers to, by TextureId
    Scene *-- SceneObject
    Scene *-- Texture
    SceneObject *-- Body
    SceneObject *-- Visual
```

## 4. Run time: who owns whom, who calls whom

```mermaid
classDiagram
    direction TB

    class Game {
        <<trait>>
        physics_dt() f64
        scene_loaded(world)
        fixed_update(world, forces)
        input(event) Option~Command~
        view(world, frame_time) View
        write_hud(world, out)
    }
    class RenderBackend {
        <<trait>>
        screen() Screen
        set_background(color)
        set_sun(direction)
        set_camera(eye, target)
        create_texture(texture) GpuTexture
        create_mesh(mesh, color, texture) ProxyId
        set_position(proxy, position)
        screen_text(text, at, size, color)
    }

    class Engine~G~ {
        game: G
        scene: Scene
        new(backend, new_game)
        input(raw) Flow
        frame(backend, frame_time)
        debug_delay() Duration
    }
    class Headless~G~ {
        new(new_game)
        input(event) Flow
        frame(frame_time)
        world() WorldView
        game() G
    }
    class NullBackend {
        draws nothing
        hands out real IDs
    }
    class Simulation {
        world: World
        clock: FixedTimestep
        entities: Interpolated per EntityId
        forces per EntityId
        load(scene, dt) Result
        advance(frame_time, before_step) steps
        view() WorldView
    }
    class Interpolated {
        body: BodyId
        previous: DVec3
        drawn: DVec3
    }
    class SceneError {
        entity: EntityId
        error: WorldError
    }
    class FixedTimestep {
        advance(frame_time) steps
        alpha() f64
    }
    class Renderer {
        proxies: ProxyId per EntityId
        new(backend, scene)
        draw(backend, world, view)
    }
    class Hud {
        begin() String
        draw(backend)
    }
    class FrameStats {
        record(frame_time)
        fps() Option~f64~
    }
    class Mouse {
        translate(raw, scale) Option~InputEvent~
    }
    class WorldView {
        time: f64
        body(entity) Option~Body~
        drawn(entity) Option~DVec3~
    }
    class Forces {
        add(entity, force)
    }
    class View {
        eye: DVec3
        target: DVec3
    }

    Engine *-- Simulation
    Engine *-- Renderer
    Engine *-- Hud
    Engine *-- FrameStats
    Engine *-- Mouse
    Engine o-- Game : runs
    Headless *-- Engine
    Headless *-- NullBackend
    RenderBackend <|.. NullBackend : implements
    Simulation *-- FixedTimestep
    Simulation *-- Interpolated
    Simulation ..> SceneError : load() may fail with
    Simulation ..> WorldView : lends, read-only
    Simulation ..> Forces : lends, per step
    Renderer ..> RenderBackend : draws through
    Hud ..> RenderBackend : writes text through
    Engine ..> View : asks the game for

    namespace flightsim {
        class FlightSim {
            ball: EntityId
            camera: Camera
            held keys
            new(scene)
        }
        class Camera {
            target, mode
            yaw, pitch, distance
            jump_to(target, velocity)
            update(target, velocity, frame_time)
            orbit(drag)
            zoom_by(scroll)
            eye() DVec3
            ground_forward() DVec2
        }
        class CameraMode {
            <<enum>>
            Orbit
            Chase
            Fixed
        }
        class GridColors {
            ground, minor, major
        }
    }
    Game <|.. FlightSim : implements
    FlightSim *-- Camera
    Camera *-- CameraMode
    FlightSim ..> WorldView : reads
    FlightSim ..> Forces : pushes the ball with
    FlightSim ..> View : returns
    FlightSim ..> GridColors : draws its grid texture with

    namespace engine_kiss3d {
        class Kiss3dBackend {
            window, scene, camera, font
            proxies: Arena of ProxyId
            textures: Arena of GpuTexture
        }
    }
    RenderBackend <|.. Kiss3dBackend : implements
```

Input types, in `engine::input`:

```mermaid
classDiagram
    direction LR
    class RawInput {
        <<enum>>
        KeyPressed(Key)
        KeyReleased(Key)
        Scroll(pixels)
        MouseButton(button, pressed)
        CursorMoved(position)
    }
    class InputEvent {
        <<enum>>
        KeyPressed(Key)
        KeyReleased(Key)
        Scroll(pixels)
        MouseDragged(button, delta)
    }
    class Command {
        <<enum>>
        Quit
        ReloadScene
        ToggleSlowFrames
        ToggleStalledFrames
    }
    class Key {
        <<enum>>
        A..Z, Escape, Space, arrows
    }
    class MouseButton {
        <<enum>>
        Left, Right, Middle
    }
    class Flow {
        <<enum>>
        Continue
        Quit
    }
    RawInput ..> InputEvent : Mouse translates into
    InputEvent ..> Command : the game may answer with
    Command ..> Flow : Quit gives
    RawInput --> Key
    RawInput --> MouseButton
    InputEvent --> Key
    InputEvent --> MouseButton
```

## 5. Loading, then one frame, step by step

**Loading** (`Engine::new`): the engine hands the game an empty `Scene`;
the game's constructor (`FlightSim::new`) spawns into it and keeps the IDs
it's handed (its ball). The engine then loads the scene: a `World` gets a
body per entity (`Simulation::load`, validated: a `SceneError` names the
bad entity), the backend gets a proxy per entity and each texture once
(`Renderer::new`), and `scene_loaded` lets the game put its camera in
place. The engine keeps the scene: `Command::ReloadScene` loads it again,
giving the same entities, so the game's IDs stay valid. `Headless` does
all this on a `NullBackend`.

**One frame**: the platform owns the loop; each iteration it reports
input, then asks the engine for a frame. Physics runs in fixed steps (0,
1 or 2 per frame); the game acts before each one; then the game chooses
where to look from and the renderer draws.

```mermaid
sequenceDiagram
    participant P as Platform<br/>(engine_kiss3d::run)
    participant E as Engine
    participant M as Mouse
    participant G as Game<br/>(FlightSim)
    participant S as Simulation
    participant W as World<br/>(physics)
    participant R as Renderer
    participant B as RenderBackend<br/>(Kiss3dBackend)

    loop every OS event
        P->>E: input(RawInput)
        E->>M: translate(raw)
        M-->>E: InputEvent (or nothing)
        E->>G: input(event)
        Note over G: remembers keys held,<br/>moves its camera
        G-->>E: Command (or nothing)
    end
    P->>E: frame(backend, frame_time)
    E->>S: advance(frame_time, before_step)
    loop each fixed step (0, 1 or 2)
        S->>G: fixed_update(WorldView, Forces)
        Note over G: pushes the ball<br/>(Forces::add by EntityId)
        S->>W: add_force(BodyId, force) per entity
        S->>W: step(dt)
        Note over W: integrate, find contacts,<br/>bounce (impulses, 8 passes),<br/>separate overlaps
    end
    Note over S: drawn = blend of the<br/>last two physics states
    E->>G: view(WorldView, frame_time)
    G-->>E: View (eye, target)
    E->>R: draw(backend, WorldView, View)
    R->>B: set_camera, set_position per ProxyId
    E->>B: screen_text (HUD: engine lines, then the game's)
    P->>B: present the frame
```

## 6. Test-only types

Not part of the game, never compiled into it:

- `CountingAllocator` (`alloc_count` crate): counts heap allocations, for
  the zero-allocation tests of the arena, the engine and the game.
- `RecordingBackend` (`engine::render::fakes`): a fake `RenderBackend`
  recording every call, its proxies and textures in arenas as a real
  backend keeps them (Unreal tests against a "null RHI" the same way;
  ours, `NullBackend`, is a real part of the engine: `Headless`).
- `TestGame` (`engine::engine::testing`): a minimal `Game` for the engine's
  tests.
