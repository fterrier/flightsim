//! The engine core: everything a game needs that isn't specific to one
//! game. The game loop's logic (fixed steps, interpolation), the camera,
//! rendering decisions, diagnostics, and the `Game` trait a game implements
//! to plug in its content, input bindings and HUD.
//!
//! No graphics library here: rendering goes through the `RenderBackend`
//! interface, implemented by a platform crate (`engine_kiss3d`). So all of
//! this compiles and is tested without a window or a GPU.
//!
//! As in established engines (Unreal's engine modules vs game modules,
//! Bevy's crates vs your app, MonoGame's `Game` base class vs your
//! subclass), the dependency only goes one way: games depend on the engine,
//! the engine never knows which game it runs.
//!
//! The public API is deliberately small, like Bevy's `prelude` or Unity's
//! scripting API: the modules are private, and only what games and
//! platforms need is re-exported below. A game can't build its own
//! renderer or step the simulation behind the engine's back.

#[cfg(test)]
mod alloc_count;
mod camera;
mod engine;
mod hud;
mod input;
mod render;
mod scene;
mod simulation;
mod timestep;

// For games: what they implement, the content they describe, the input
// they bind, and read access to the simulation for their HUD lines.
pub use engine::Game;
pub use input::{Command, InputEvent, Key};
pub use render::{Color, Shape, Visual};
pub use scene::{Scene, SceneObject};
pub use simulation::Simulation;

// For platforms: the engine they run, and the interface they implement to
// draw with a graphics library.
pub use engine::{Engine, Flow};
pub use render::{ProxyId, RenderBackend, Screen};
