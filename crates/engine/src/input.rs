//! Input, the engine's way: its own key type, input events, and the
//! commands they can trigger.
//!
//! Games never see the platform's input types (kiss3d's keys): the platform
//! layer translates them into these. A game's *bindings* (which key does
//! what) map events to `Command`s: the Command pattern ("Game Programming
//! Patterns"), so bindings are data, and the same commands could come from
//! a replay file or an AI instead of the keyboard.

/// The keys the engine knows about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Escape, Space, Up, Down, Left, Right,
}

/// Something the player did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    KeyPressed(Key),
    /// Mouse wheel or trackpad scroll, in pixels (a wheel notch counts as
    /// 10); positive is up.
    Scroll(f64),
}

/// What the engine can be asked to do.
// TODO(step-06): flight controls (throttle, pitch), passed on to the
// simulation; and keys *held* down, not just pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Quit,
    /// Reload the scene into a fresh simulation.
    Restart,
    /// Switch the camera between following and fixed.
    ToggleCamera,
    /// Zoom by a scroll distance in pixels (positive zooms in).
    Zoom(f64),
    /// Debug: make every frame 50 ms longer, like a slow machine.
    ToggleSlowFrames,
    /// Debug: make every frame 400 ms longer, beyond the frame-time cap.
    ToggleStalledFrames,
}
