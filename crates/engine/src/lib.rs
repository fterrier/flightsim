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

#[cfg(test)]
mod alloc_count;
pub mod camera;
pub mod engine;
pub mod hud;
pub mod input;
pub mod render;
pub mod scene;
pub mod simulation;
pub mod timestep;

pub use engine::{Engine, Flow, Game};
