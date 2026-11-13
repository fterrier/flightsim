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
    engine["<b>engine</b> (ENGINE: core)<br/>Engine, Game, Scene, Renderer, RenderBackend..."]
    physics["<b>physics</b> (ENGINE: pure)<br/>World, Body, Motion, Collider"]
    alloc["<b>alloc_count</b> (TESTS only)<br/>CountingAllocator"]
    kiss3d[("kiss3d + wgpu<br/>window, GPU")]
    glam[("glam<br/>vectors")]

    flightsim -->|"game API"| engine
    flightsim -->|"run()"| kiss3d_crate
    kiss3d_crate -->|"platform API"| engine
    kiss3d_crate --> kiss3d
    engine --> physics
    physics --> glam
    engine -.->|"dev-dependency"| alloc
    flightsim -.->|"dev-dependency"| alloc
```

The `engine` crate shows two faces:

| Who | Imports | Gets |
|---|---|---|
| A **game** (`flightsim`) | `engine::...` (the crate root) | what it implements (`Game`), describes (`Scene`, `SceneObject`, `Visual`, `Shape`, `Material`, `Texture`, the physics types `Body`, `Motion`...), reads (`WorldView`), does (`Forces`, `View`, `Command`), receives (`InputEvent`, `Key`, `MouseButton`) |
| A **platform** (`engine_kiss3d`) | `engine::platform::...` | the engine it runs (`Engine`, `Flow`), the raw input it reports (`RawInput`), the interface it implements (`RenderBackend`) and what that interface carries (`Mesh`, `ProxyId`, `GpuTexture`, `Screen`, `VIEW_DISTANCE`) |

Everything else in the engine is `pub(crate)`: machinery no one outside
can touch (`Simulation`, `Renderer`, `Hud`...). CI checks the game never
imports `engine::platform`.

## 2. Data: what a scene describes, and what the physics simulates

A **scene** is a description, written by the game. Loading it splits each
object: its `Body` goes to the physics `World`, its `Visual` to the
`Renderer`. Object `i` is body `i` and render proxy `i`.

```mermaid
classDiagram
    direction LR

    class Scene {
        objects: Vec~SceneObject~
        textures: Vec~Texture~
        background: Color
        gravity: DVec3
    }
    class SceneObject {
        body: Body
        visual: Visual
    }

    namespace physics {
        class World {
            gravity: DVec3
            bodies: Vec~Body~
            new(bodies, gravity) Result
            step(dt, forces)
            time() f64
        }
        class WorldError {
            <<enum>>
            MustBeStatic
            InvalidMass
        }
        class Body {
            body_type: BodyType
            motion: Motion
            collider: Collider
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
    World *-- Body
    World ..> WorldError : new() may fail with
    Body *-- BodyType
    Body *-- Motion
    Body *-- Collider

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
    class TextureId {
        index in Scene.textures
    }
    class Texture {
        width, height
        srgba: Vec~u8~
        from_fn(w, h, pixel)
    }
    class Color {
        r, g, b, a: linear light
    }
    Visual *-- Shape
    Visual *-- Material
    Shape *-- PlaneSize
    Material *-- Color
    Material o-- TiledTexture
    TiledTexture --> TextureId
    TextureId ..> Texture : refers to

    Scene *-- SceneObject
    Scene *-- Texture
    SceneObject *-- Body
    SceneObject *-- Visual
```

## 3. Run time: who owns whom, who calls whom

```mermaid
classDiagram
    direction TB

    class Game {
        <<trait>>
        physics_dt() f64
        scene() Scene
        scene_loaded(world)
        fixed_update(bodies, forces)
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
        start: World
        new(backend, game)
        input(raw) Flow
        frame(backend, frame_time)
        debug_delay() Duration
    }
    class Simulation {
        world: World
        clock: FixedTimestep
        previous, drawn, forces
        advance(frame_time, before_step) steps
        view() WorldView
    }
    class FixedTimestep {
        advance(frame_time) steps
        alpha() f64
    }
    class Renderer {
        proxies: Vec~ProxyId~
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
        bodies: slice of Body
        drawn: slice of DVec3
    }
    class Forces {
        add(body, force)
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
    Simulation *-- FixedTimestep
    Simulation ..> WorldView : lends, read-only
    Simulation ..> Forces : lends, per step
    Renderer ..> RenderBackend : draws through
    Hud ..> RenderBackend : writes text through
    Engine ..> View : asks the game for

    namespace flightsim {
        class FlightSim {
            focus: usize
            camera: Camera
            held keys
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
            window, scene, camera
            proxies, textures, font
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
        Restart
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

## 4. One frame, step by step

The platform owns the loop; each iteration it reports input, then asks the
engine for a frame. Physics runs in fixed steps (0, 1 or 2 per frame); the
game acts before each one; then the game chooses where to look from and
the renderer draws.

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
        S->>G: fixed_update(bodies, forces)
        Note over G: pushes the ball<br/>(Forces::add)
        S->>W: step(dt, forces)
    end
    Note over S: drawn = blend of the<br/>last two physics states
    E->>G: view(WorldView, frame_time)
    G-->>E: View (eye, target)
    E->>R: draw(backend, WorldView, View)
    R->>B: set_camera, set_position per proxy
    E->>B: screen_text (HUD: engine lines, then the game's)
    P->>B: present the frame
```

At load (`Engine::new`), before the first frame: the game's `scene()` is
turned into a `World` (validated) and render proxies (`Renderer::new`:
`create_texture`, `create_mesh`), then `scene_loaded` lets the game put
its camera in place. `Restart` rebuilds the `Simulation` from the saved
start `World` and calls `scene_loaded` again.

## 5. Test-only types

Not part of the game, never compiled into it:

- `CountingAllocator` (`alloc_count` crate): counts heap allocations, for
  the zero-allocation tests of the engine and the game.
- `RecordingBackend`, `NullBackend` (`engine::render::fakes`): fake
  `RenderBackend`s, the first recording every call, the second ignoring
  them (Unreal's "null RHI" idea).
- `TestGame` (`engine::engine::testing`): a minimal `Game` for the engine's
  tests.
