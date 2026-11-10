//! The HUD (heads-up display): a text overlay drawn in screen space
//! (physical pixels, origin top-left, y down), on top of the world, like
//! Unreal's `AHUD` or a Unity canvas. The engine writes its diagnostics
//! into it and the game appends its own lines (`Game::write_hud`).
//!
//! The text is rebuilt every frame into one `String` that is reused, never
//! reallocated: its capacity is reserved once, up front. Allocating fresh
//! strings every frame (as `format!` does) is a classic source of wasted
//! time and memory churn in game loops; the `alloc_count` test checks a
//! whole frame allocates nothing.

use glam::Vec2;

use crate::render::{RenderBackend, WHITE};

/// FPS is averaged over windows of this many seconds. A per-frame value
/// (1 / last frame time) jumps around with every hiccup and is unreadable.
const FPS_WINDOW: f64 = 0.5;

/// Bytes reserved for the HUD text: comfortably more than it ever needs,
/// so the buffer never has to grow (which would allocate).
const TEXT_CAPACITY: usize = 1024;

/// Frame diagnostics: the frame rate, averaged over `FPS_WINDOW`, and the
/// real (wall-clock) time since the simulation (re)started. That's Bevy's
/// `Time<Real>`, as opposed to the simulation's fixed-step clock: compare
/// the two to see whether the simulation keeps up.
#[derive(Default)]
pub struct FrameStats {
    /// Frames and seconds counted in the current window.
    frames: u32,
    elapsed: f64,
    /// Average over the last complete window; `None` before the first one.
    fps: Option<f64>,
    /// Seconds of real time since the simulation (re)started.
    real_time: f64,
}

impl FrameStats {
    /// Records one frame that lasted `frame_time` seconds.
    pub fn record(&mut self, frame_time: f64) {
        self.real_time += frame_time;
        self.frames += 1;
        self.elapsed += frame_time;
        if self.elapsed >= FPS_WINDOW {
            self.fps = Some(self.frames as f64 / self.elapsed);
            self.frames = 0;
            self.elapsed = 0.0;
        }
    }

    /// Average frames per second over the last complete window.
    pub fn fps(&self) -> Option<f64> {
        self.fps
    }

    /// Seconds of real time since the simulation (re)started.
    pub fn real_time(&self) -> f64 {
        self.real_time
    }

    /// Restarts the real-time clock, when the simulation restarts.
    pub fn restart_clock(&mut self) {
        self.real_time = 0.0;
    }
}

pub struct Hud {
    text: String,
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            text: String::with_capacity(TEXT_CAPACITY),
        }
    }
}

impl Hud {
    /// Starts this frame's text: empties the buffer (keeping its memory,
    /// so nothing is allocated) and returns it to write lines into.
    pub fn begin(&mut self) -> &mut String {
        self.text.clear();
        &mut self.text
    }

    /// The HUD text, line by line, top to bottom.
    pub fn lines(&self) -> std::str::Lines<'_> {
        self.text.lines()
    }

    /// Draws the text in the top-left corner, one line under another.
    pub fn draw(&self, backend: &mut impl RenderBackend) {
        let scale = backend.screen().scale;
        for (i, line) in self.lines().enumerate() {
            let (x, y) = line_position(i, scale);
            backend.text(line, Vec2::new(x, y), font_size(scale), WHITE);
        }
    }
}

/// HUD text size, in physical pixels.
fn font_size(scale: f32) -> f32 {
    18.0 * scale
}

/// Screen position (physical pixels) of HUD text line number `line`.
fn line_position(line: usize, scale: f32) -> (f32, f32) {
    (12.0 * scale, (12.0 + 21.0 * line as f32) * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Screen;
    use crate::render::fakes::RecordingBackend;

    /// Exact in binary floating point.
    const FRAME: f64 = 1.0 / 64.0;

    const SCREEN: Screen = Screen {
        width: 1000.0,
        height: 600.0,
        scale: 1.0,
    };

    #[test]
    fn no_fps_until_a_full_window_has_been_measured() {
        let mut stats = FrameStats::default();
        assert_eq!(stats.fps(), None);
        for _ in 0..31 {
            stats.record(FRAME); // 31/64 s: not yet half a second
        }
        assert_eq!(stats.fps(), None);
        stats.record(FRAME);
        assert_eq!(stats.fps(), Some(64.0));
    }

    #[test]
    fn fps_is_an_average_so_one_slow_frame_does_not_make_it_jump() {
        let mut stats = FrameStats::default();
        for _ in 0..28 {
            stats.record(FRAME);
        }
        stats.record(4.0 * FRAME); // one 62.5 ms hiccup: instantly "16 FPS"
        // 29 frames in 32/64 s = 58 FPS on average.
        assert_eq!(stats.fps(), Some(58.0));
    }

    #[test]
    fn real_time_adds_up_frame_times_and_restarts_with_the_simulation() {
        let mut stats = FrameStats::default();
        for _ in 0..16 {
            stats.record(FRAME);
        }
        assert_eq!(stats.real_time(), 0.25);
        stats.restart_clock();
        assert_eq!(stats.real_time(), 0.0);
    }

    #[test]
    fn text_is_replaced_not_appended_each_frame() {
        let mut hud = Hud::default();
        hud.begin().push_str("one\ntwo");
        hud.begin().push_str("three");
        assert_eq!(hud.lines().collect::<Vec<_>>(), ["three"]);
    }

    #[test]
    fn lines_are_drawn_top_to_bottom_in_the_top_left_corner() {
        let mut hud = Hud::default();
        hud.begin().push_str("first\nsecond");
        let mut backend = RecordingBackend::new(SCREEN);
        hud.draw(&mut backend);
        let texts: Vec<&str> = backend.texts.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(texts, ["first", "second"]);
        let (first, second) = (backend.texts[0].1, backend.texts[1].1);
        assert_eq!(first.x, second.x);
        assert!(second.y > first.y); // screen y grows downwards
    }

    #[test]
    fn text_is_twice_as_big_on_a_retina_display() {
        let retina = Screen {
            width: 2000.0,
            height: 1200.0,
            scale: 2.0,
        };
        let mut hud = Hud::default();
        hud.begin().push('x');
        let (mut normal, mut double) =
            (RecordingBackend::new(SCREEN), RecordingBackend::new(retina));
        hud.draw(&mut normal);
        hud.draw(&mut double);
        assert_eq!(double.texts[0].2, 2.0 * normal.texts[0].2);
        assert_eq!(double.texts[0].1, 2.0 * normal.texts[0].1);
    }
}
