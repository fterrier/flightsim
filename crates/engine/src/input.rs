//! Input, the engine's way: its own key and button types, input events,
//! and the commands they can trigger.
//!
//! Two levels, as in engines (Godot's OS events vs `InputEvent`s, Bevy's
//! winit events vs `bevy_input`):
//! - the platform reports what the operating system says, as [`RawInput`]
//!   (the cursor is *at* this position), without any logic;
//! - the engine turns that into the [`InputEvent`]s games bind (the mouse
//!   was *dragged* by this much), keeping the state that needs (which
//!   button is held, where the cursor was).
//!
//! A game's *bindings* (which key does what) map events to `Command`s: the
//! Command pattern ("Game Programming Patterns"), so bindings are data, and
//! the same commands could come from a replay file or an AI instead of the
//! keyboard.

use physics::DVec2;

/// The keys the engine knows about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Escape, Space, Up, Down, Left, Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// What the operating system reported, as the platform passes it on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawInput {
    KeyPressed(Key),
    KeyReleased(Key),
    /// Mouse wheel or trackpad scroll, in pixels (a wheel notch counts as
    /// 10); positive is up.
    Scroll(f64),
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    /// The cursor is now at this position: physical pixels from the
    /// window's top-left corner, y down.
    CursorMoved(DVec2),
}

/// Something the player did, as games see it. A key held down is pressed
/// once, then released once: what's held in between is for the game to
/// remember (as Bevy's `ButtonInput` or Godot's `is_action_pressed` do).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    KeyPressed(Key),
    KeyReleased(Key),
    /// Mouse wheel or trackpad scroll, in pixels (a wheel notch counts as
    /// 10); positive is up.
    Scroll(f64),
    /// The mouse moved by `delta` while `button` was held: logical pixels
    /// (the same on any screen density), x right, y down. Like Godot's
    /// mouse motion `relative`.
    MouseDragged {
        button: MouseButton,
        delta: DVec2,
    },
}

/// What a game can ask the engine to do: the engine's own services.
/// Everything else input does (moving the camera, pushing things, later
/// flying) is the game's business, in `Game::input`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Quit,
    /// Load the scene again from its description, everything back where
    /// it started (Godot's `reload_current_scene`, Unity's `LoadScene` of
    /// the current scene). When to do it ("press R to restart") is the
    /// game's choice.
    ReloadScene,
    /// Debug: make every frame 50 ms longer, like a slow machine.
    ToggleSlowFrames,
    /// Debug: make every frame 400 ms longer, beyond the frame-time cap.
    ToggleStalledFrames,
}

/// Turns raw mouse reports into drags: remembers where the cursor was and
/// which button is held.
#[derive(Debug, Default)]
pub(crate) struct Mouse {
    /// Last known cursor position, in physical pixels (none before the
    /// first report).
    cursor: Option<DVec2>,
    /// The button that started the current drag. One at a time: pressing
    /// another one meanwhile doesn't change it.
    dragging: Option<MouseButton>,
}

impl Mouse {
    /// The game-level event for `raw`, if any. `scale` is the screen's
    /// physical pixels per logical pixel (2 on a Retina display).
    pub(crate) fn translate(&mut self, raw: RawInput, scale: f64) -> Option<InputEvent> {
        match raw {
            RawInput::KeyPressed(key) => Some(InputEvent::KeyPressed(key)),
            RawInput::KeyReleased(key) => Some(InputEvent::KeyReleased(key)),
            RawInput::Scroll(amount) => Some(InputEvent::Scroll(amount)),
            RawInput::MouseButton { button, pressed } => {
                if pressed && self.dragging.is_none() {
                    self.dragging = Some(button);
                } else if !pressed && self.dragging == Some(button) {
                    self.dragging = None;
                }
                None
            }
            RawInput::CursorMoved(position) => {
                let previous = self.cursor.replace(position)?;
                let button = self.dragging?;
                let delta = (position - previous) / scale;
                Some(InputEvent::MouseDragged { button, delta })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(button: MouseButton) -> RawInput {
        RawInput::MouseButton {
            button,
            pressed: true,
        }
    }

    fn release(button: MouseButton) -> RawInput {
        RawInput::MouseButton {
            button,
            pressed: false,
        }
    }

    fn cursor(x: f64, y: f64) -> RawInput {
        RawInput::CursorMoved(DVec2::new(x, y))
    }

    #[test]
    fn keys_and_scrolling_pass_through() {
        let mut mouse = Mouse::default();
        assert_eq!(
            mouse.translate(RawInput::KeyPressed(Key::C), 1.0),
            Some(InputEvent::KeyPressed(Key::C))
        );
        assert_eq!(
            mouse.translate(RawInput::KeyReleased(Key::C), 1.0),
            Some(InputEvent::KeyReleased(Key::C))
        );
        assert_eq!(
            mouse.translate(RawInput::Scroll(-10.0), 1.0),
            Some(InputEvent::Scroll(-10.0))
        );
    }

    #[test]
    fn moving_with_a_button_held_drags_by_the_distance_moved() {
        let mut mouse = Mouse::default();
        mouse.translate(cursor(10.0, 10.0), 1.0);
        assert_eq!(mouse.translate(press(MouseButton::Left), 1.0), None);
        assert_eq!(
            mouse.translate(cursor(30.0, 5.0), 1.0),
            Some(InputEvent::MouseDragged {
                button: MouseButton::Left,
                delta: DVec2::new(20.0, -5.0),
            })
        );
    }

    #[test]
    fn drags_are_in_logical_pixels() {
        // On a Retina display (2 physical pixels per logical one), the same
        // hand movement reports twice the pixels: it must drag as far.
        let mut mouse = Mouse::default();
        mouse.translate(press(MouseButton::Right), 2.0);
        mouse.translate(cursor(0.0, 0.0), 2.0);
        assert_eq!(
            mouse.translate(cursor(40.0, 20.0), 2.0),
            Some(InputEvent::MouseDragged {
                button: MouseButton::Right,
                delta: DVec2::new(20.0, 10.0),
            })
        );
    }

    #[test]
    fn moving_without_a_button_held_is_not_a_drag() {
        let mut mouse = Mouse::default();
        mouse.translate(cursor(0.0, 0.0), 1.0);
        assert_eq!(mouse.translate(cursor(5.0, 5.0), 1.0), None);
    }

    #[test]
    fn the_first_cursor_report_has_nothing_to_compare_with() {
        let mut mouse = Mouse::default();
        mouse.translate(press(MouseButton::Left), 1.0);
        assert_eq!(mouse.translate(cursor(100.0, 100.0), 1.0), None);
    }

    #[test]
    fn releasing_the_button_ends_the_drag() {
        let mut mouse = Mouse::default();
        mouse.translate(cursor(0.0, 0.0), 1.0);
        mouse.translate(press(MouseButton::Left), 1.0);
        mouse.translate(release(MouseButton::Left), 1.0);
        assert_eq!(mouse.translate(cursor(5.0, 5.0), 1.0), None);
    }

    #[test]
    fn the_button_that_started_the_drag_owns_it() {
        let mut mouse = Mouse::default();
        mouse.translate(cursor(0.0, 0.0), 1.0);
        mouse.translate(press(MouseButton::Left), 1.0);
        mouse.translate(press(MouseButton::Right), 1.0);
        mouse.translate(release(MouseButton::Right), 1.0); // not its drag
        assert_eq!(
            mouse.translate(cursor(1.0, 0.0), 1.0),
            Some(InputEvent::MouseDragged {
                button: MouseButton::Left,
                delta: DVec2::new(1.0, 0.0),
            })
        );
    }
}
