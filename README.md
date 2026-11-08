[![CI](https://github.com/fterrier/flightsim/actions/workflows/ci.yml/badge.svg)](https://github.com/fterrier/flightsim/actions/workflows/ci.yml)

# flightsim

A flight simulator written from scratch in Rust, as a learning project.

## Goals

1. **Learn how games and game engines work**: game loops, fixed timesteps,
   memory layout and ownership, entity management, rendering pipelines.
2. **High-fidelity flight physics**: a real 6-degrees-of-freedom rigid-body
   model with aerodynamics, propulsion and ground contact, validated against
   real aircraft data.
3. **Generic aircraft**: aircraft are *data* (a definition file), not code.
   We start with a Cessna 152 and add more later.
4. **A world, not just a plane**: eventually multiple aircraft, buildings and
   other objects sharing the same simulation.
5. **Clean separation** between physics, collision, input, and rendering, so
   each can be understood, tested and replaced on its own.
6. **Iterate**: small runnable steps. Physics first with rudimentary
   graphics, then accelerated 3D graphics.

### Non-goals (for now)

- Pretty graphics, sound, multiplayer, real-world scenery.
- Using a full game engine (Bevy, Unity, Unreal...): they hide exactly the
  parts this project is meant to teach.

## Principles

- **Simplicity first.** Plain structs and functions. No abstraction until a
  second use case actually exists.
- **Physics is pure.** The `physics` crate has no clock, no I/O and no
  graphics: given the same state, inputs and `dt`, it always produces the
  same result. That makes it testable and replayable.
- **SI units everywhere** (metres, seconds, kilograms, radians), `f64` for
  physics.
- **Every step is runnable, tested and tagged** (`step-00`, `step-01`, ...),
  so `git diff step-03 step-04` shows exactly what one step taught.

The full plan, step by step, is in [ROADMAP.md](ROADMAP.md).

## Layout

```
crates/
  physics/   library: simulation state + update functions (pure, no graphics)
  app/       binary `flightsim`: window, input, game loop, drawing
```

## Run

1. Install Rust via [rustup](https://rustup.rs) (once per machine):

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

   Open a new terminal afterwards so `cargo` is on your `PATH`. You don't
   need to pick a Rust version: the one pinned in `rust-toolchain.toml` is
   downloaded automatically the first time you build.

2. Get the code and run the simulator:

   ```sh
   git clone git@github.com:fterrier/flightsim.git
   cd flightsim
   cargo run -p app
   ```

   The first build downloads and compiles dependencies, so it takes a bit
   longer. For a faster simulation, build with optimisations:
   `cargo run -p app --release`.

What you see depends on how far the [ROADMAP](ROADMAP.md) has got. To run
the program as it was after a given step:

```sh
git checkout step-00      # any step tag: step-00, step-01, ...
cargo run -p app
git checkout master       # back to the latest version
```

### Controls

| Key | Action |
|---|---|
| `Esc` | Quit |
| `L` (hold) | Debug: slow frames (+50 ms each, ~15 FPS). The simulation keeps real-time pace. |
| `K` (hold) | Debug: stalled frames (+400 ms each). Frame time is capped, so sim time falls behind real time. |

Each step that adds controls updates this table.

## Develop

```sh
cargo test --workspace                                 # run all tests
cargo fmt --all                                        # format code
cargo clippy --workspace --all-targets -- -D warnings  # lint
cargo llvm-cov --workspace --ignore-filename-regex 'crates/app/src/main\.rs'  # coverage
```

Coverage needs `cargo install cargo-llvm-cov` once.

CI (`.github/workflows/ci.yml`) runs `fmt --check`, `clippy` and `test` on
Linux and macOS, and enforces ≥ 95% line coverage, on every push.

Development practices (TDD, coverage, what to run before committing) are in
[AGENTS.md](AGENTS.md).

## History

The first attempt was in C++ (CMake, OpenSceneGraph, then Magnum). It is
preserved at the git tag `cpp-legacy` (branch `legacy`).

## License

[Eclipse Public License 2.0](LICENSE)
