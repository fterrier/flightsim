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

The only excluded file is `crates/app/src/main.rs`: it is the thin shell
that opens the window, reads the real keyboard and draws. It can't run in
CI (no display), so it must contain **no logic**: anything worth testing
(timing, input mapping, coordinate conversions) goes into a tested module
and `main.rs` just calls it.

Don't game coverage: a test that executes code without asserting anything
meaningful doesn't count.

## Before every commit

All four must pass (CI runs the same):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --fail-under-lines 95 --ignore-filename-regex 'crates/app/src/main\.rs'
```

Also run the app (`cargo run -p app`) when the step changes what's on
screen.

## Code rules

- **Physics is pure**: the `physics` crate has no clock, I/O, input devices
  or graphics. Time enters only as a `dt` argument.
- **Physics is ECS-agnostic**: physics functions take components
  (`&mut Body`), never a `World` or ECS query (see ROADMAP "Entities").
- **SI units** (m, s, kg, rad), `f64` in physics. Name units in doc
  comments. Convert to pixels / `f32` only at the drawing boundary.
- No `unwrap()`/`expect()` in library code paths that can fail at runtime;
  fine in tests.

## Git

- Commit messages: `Step N: <summary>` for step commits, imperative mood,
  body explains why.
- Never push or rewrite pushed history without the owner's OK.
