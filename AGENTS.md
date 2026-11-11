# Working on flightsim

Instructions for anyone (human or AI agent) changing this code.
Project goals are in [README.md](README.md), the step-by-step plan in
[ROADMAP.md](ROADMAP.md).

## This is a learning project

The owner learns by reading the code. Optimise for **readability and
simplicity** over cleverness or generality.

- One ROADMAP step at a time. When a step is done: tests green, coverage
  met, ROADMAP status updated, commit, tag `step-NN`, then **stop** and let
  the owner test and review before starting the next step.
- Keep each step's diff small and focused on the concept it teaches.
- Comments explain *why* (the physics, the trade-off), not *what*.
- Plain structs and free functions. No traits, generics or new crates until
  a second concrete use exists.

### TODO comments: where future steps plug in

When code will be extended by a later ROADMAP step, leave a `TODO` comment
at the exact spot, so the reader sees where the system is heading and how
the pieces will connect:

```rust
// TODO(step-06): read throttle and pitch keys into controls and pass
// them to `world.step`.
if window.get_key(Key::L) == Action::Press {
```

- Format: `// TODO(step-NN): <what plugs in here> [and why here]`. Always
  name the step from ROADMAP.md, so `grep -rn "TODO(step-06)"` finds every
  place a step touches.
- Use `TODO(later)` for items in the ROADMAP "Later" list.
- Comment only, no placeholder code: no empty functions, stub structs or
  `todo!()` for future steps. The comment explains; the code stays minimal
  and fully tested.
- When implementing a step, find its TODOs first, then remove each one as
  it's done. A step isn't finished while its TODOs remain.
- Don't add TODOs for ordinary unfinished work or known bugs in the current
  step: finish it, or add it to the ROADMAP.

## Test-driven development

Every change is tested. Work in red-green-refactor cycles:

1. **Red**: write a test describing the behaviour you want. Run it, watch
   it fail (for the right reason).
2. **Green**: write the simplest code that makes it pass.
3. **Refactor**: clean up with the tests as a safety net.

Bug fixes start with a test that reproduces the bug.

### What good tests look like here

- **Physics against analytic solutions**: free fall `y = y0 + v0 t - g t²/2`,
  constant velocity, steady glide, ...
- **Invariants**: energy / momentum conservation where applicable,
  quaternions stay normalised, determinism (same inputs => bit-identical
  state).
- **Real-world validation** (from step 6): published aircraft performance
  (stall speed, climb rate) within a stated tolerance.
- Floating point: compare with an explicit tolerance and say why that
  tolerance (e.g. integration error is O(dt)).
- Unit tests live next to the code (`#[cfg(test)] mod tests`).

### Coverage: at least 95% of lines

Enforced in CI with [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov).

Excluded, because they need a real window and GPU, which CI doesn't
have:

- the `engine_kiss3d` crate: the platform layer (window, run loop, input
  translation) and the `RenderBackend` implemented with kiss3d, one
  kiss3d call per method;
- `crates/flightsim/src/main.rs`: one line, `engine_kiss3d::run(...)`.

Both are "humble objects" (Meszaros, *xUnit Test Patterns*): they must
contain **no logic**. Anything that decides something (timing, input
mapping, coordinate conversions, what to draw where) goes into a tested
module. Rendering decisions live in `render.rs` and are tested against
`RecordingBackend` / `NullBackend`, fakes of the backend interface (the
idea behind Unreal's "null RHI").

Don't game coverage: a test that executes code without asserting anything
meaningful doesn't count.

## Before every commit

All four must pass (CI runs the same):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --fail-under-lines 95 --ignore-filename-regex 'crates/(engine_kiss3d/|flightsim/src/main\.rs)'
```

Also run the game (`cargo run -p flightsim`) when the step changes what's on
screen.

## Code rules

- **Physics is pure**: the `physics` crate has no clock, I/O, input devices
  or graphics. Time enters only as a `dt` argument.
- **Physics is ECS-agnostic**: physics functions take components
  (`&mut Motion`, `&Circle`), never a `World` or ECS query (see ROADMAP "Entities").
- **SI units** (m, s, kg, rad), `f64` in physics. Name units in doc
  comments. Convert to pixels / `f32` only at the drawing boundary.
- No `unwrap()`/`expect()` in library code paths that can fail at runtime;
  fine in tests.
- **No heap allocation per frame** in the game loop: iterate instead of
  collecting into temporary `Vec`s, reuse buffers (`String::clear` keeps
  capacity). Anything new in the loop goes into a zero-allocation test:
  the engine's (`crates/engine/src/no_alloc.rs`) or, for game code run
  every frame (input, camera, HUD), the game's
  (`crates/flightsim/src/no_alloc.rs`). Both count with the `alloc_count`
  test-only crate.
- Separate data by concept, as engines do: a body (motion) is not its
  collider (shape); simulation state is not render state.
- **Engine vs game**: the `physics`, `engine` and `engine_kiss3d` crates
  must never know which game they run (no scenario, key binding or
  game-specific text in them). Game-specific code goes in `flightsim`,
  plugged in through the `engine::Game` trait.
- **Curated public API**: `engine`'s modules are private. The crate root
  re-exports the *game* API (including the physics content types: games
  depend on `engine` only); `engine::platform` holds what *platform*
  crates need (`Engine`, `RenderBackend`...). Games never import
  `engine::platform` (CI checks it). Expose something new only when a game
  or platform genuinely needs it, and say why.
- **Least visibility**: private by default; `pub(crate)` when another
  module of the crate needs it; `pub` only for real public API. The
  `unreachable_pub` lint (workspace-wide) rejects a `pub` that other crates
  can't reach. Games get *data* (`WorldView`: a read-only borrow of the
  world, no copy), never engine machinery (`World::step`, `Simulation`,
  `Renderer`). What a game decides (its controls, its camera) is game
  code: the engine asks (`Game::input`, `Game::view`), it doesn't decide.

## Git

- Commit messages: `Step N: <summary>` for step commits, imperative mood,
  body explains why.
- Never push or rewrite pushed history without the owner's OK.
