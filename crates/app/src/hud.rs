//! The HUD (heads-up display): debug text drawn in screen space (physical
//! pixels, origin top-left, y down), on top of the world.
//!
//! The text is rebuilt every frame into one `String` that is reused, never
//! reallocated: its capacity is reserved once, up front. Allocating fresh
//! strings every frame (as `format!` does) is a classic source of wasted
//! time and memory churn in game loops; the `alloc_count` test checks a
//! whole frame allocates nothing.

use std::fmt::Write;

use crate::camera::CameraMode;
use crate::simulation::Simulation;

/// FPS is averaged over windows of this many seconds. A per-frame value
/// (1 / last frame time) jumps around with every hiccup and is unreadable.
const FPS_WINDOW: f64 = 0.5;

/// Bytes reserved for the HUD text: comfortably more than it ever needs,
/// so the buffer never has to grow (which would allocate).
const TEXT_CAPACITY: usize = 1024;

/// Measures the frame rate, averaged over `FPS_WINDOW`.
#[derive(Default)]
pub struct FrameStats {
    /// Frames and seconds counted in the current window.
    frames: u32,
    elapsed: f64,
    /// Average over the last complete window; `None` before the first one.
    fps: Option<f64>,
}

impl FrameStats {
    /// Records one frame that lasted `frame_time` seconds.
    pub fn record(&mut self, frame_time: f64) {
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
    /// Rewrites the HUD text for this frame.
    ///
    /// Watch "sim time" vs "real time": they stay equal at any frame rate,
    /// until frames exceed `MAX_FRAME_TIME` and the simulation falls behind.
    /// `focus` is the index of the body the HUD describes.
    pub fn update(
        &mut self,
        sim: &Simulation,
        focus: usize,
        steps: u32,
        stats: &FrameStats,
        camera_mode: CameraMode,
    ) {
        // `clear` keeps the capacity: the same memory is reused every frame.
        self.text.clear();
        let t = &mut self.text;
        // Writing into a `String` only fails if memory runs out, which
        // aborts the program anyway, so the `fmt::Result` can't be an error.
        let ok = "writing to a String cannot fail";

        match stats.fps() {
            // Frame time (ms) is what developers watch: it adds up across
            // a frame's work, while FPS doesn't.
            Some(fps) => writeln!(t, "{fps:.0} FPS ({:.1} ms per frame)", 1000.0 / fps),
            None => writeln!(t, "-- FPS"),
        }
        .expect(ok);

        let world = sim.world();
        let body = &world.bodies[focus];
        let motion = &body.motion;
        write!(
            t,
            "{steps} physics steps this frame\n\
             drawn {:.0}% of the way to the next step\n\
             sim time  {:5.2} s\n\
             real time {:5.2} s\n\
             x = {:.1} m\n\
             height = {:.1} m\n\
             vertical speed = {:.1} m/s\n\
             camera: {} (C to switch)\n\
             R: throw again | hold L: slow frames | hold K: stalled frames | \
             scroll: zoom | Esc: quit",
            sim.clock().alpha() * 100.0,
            world.time,
            sim.clock().real_time(),
            motion.position.x,
            // Height of the bottom of the ball, so it reads 0 when resting.
            motion.position.y - body.collider.radius,
            motion.velocity.y,
            camera_mode.name(),
        )
        .expect(ok);
    }

    /// The HUD text, line by line, top to bottom.
    pub fn lines(&self) -> std::str::Lines<'_> {
        self.text.lines()
    }
}

/// HUD text size, in physical pixels.
pub fn font_size(scale: f32) -> f32 {
    18.0 * scale
}

/// Screen position (physical pixels) of HUD text line number `line`.
pub fn line_position(line: usize, scale: f32) -> (f32, f32) {
    (12.0 * scale, (12.0 + 21.0 * line as f32) * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{Body, Circle, DVec2, Motion, World};

    /// Exact in binary floating point.
    const FRAME: f64 = 1.0 / 64.0;

    /// A ball of radius 0.4 m.
    fn ball(position: DVec2, velocity: DVec2) -> Body {
        Body {
            motion: Motion { position, velocity },
            collider: Circle { radius: 0.4 },
        }
    }

    #[test]
    fn hud_describes_the_focused_object() {
        let still = ball(DVec2::new(0.0, 0.4), DVec2::ZERO);
        let far = ball(DVec2::new(42.0, 0.4), DVec2::ZERO);
        let sim = Simulation::new(World::new(vec![still, far]), FRAME);
        let mut hud = Hud::default();
        hud.update(&sim, 1, 0, &FrameStats::default(), CameraMode::Follow);
        let text: Vec<&str> = hud.lines().collect();
        assert!(text.contains(&"x = 42.0 m"), "{text:?}");
    }

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
    fn hud_shows_timing_position_speed_and_camera() {
        // A ball resting on the ground (radius 0.4 m), sliding at 10 m/s.
        let ball = ball(DVec2::new(0.0, 0.4), DVec2::new(10.0, 0.0));
        let mut sim = Simulation::new(World::new(vec![ball]), FRAME);
        // 0.25 s = exactly 16 steps, nothing left over.
        let steps = sim.advance(0.25);
        let mut stats = FrameStats::default();
        for _ in 0..32 {
            stats.record(FRAME);
        }

        let mut hud = Hud::default();
        hud.update(&sim, 0, steps, &stats, CameraMode::Follow);
        let text: Vec<&str> = hud.lines().collect();
        let text = text.join("\n");
        assert!(text.contains("64 FPS (15.6 ms per frame)"), "{text}");
        assert!(text.contains("16 physics steps this frame"), "{text}");
        assert!(
            text.contains("drawn 0% of the way to the next step"),
            "{text}"
        );
        assert!(text.contains("sim time   0.25 s"), "{text}");
        assert!(text.contains("real time  0.25 s"), "{text}");
        assert!(text.contains("x = 2.5 m"), "{text}");
        assert!(text.contains("height = 0.0 m"), "{text}"); // above ground
        assert!(text.contains("vertical speed = 0.0 m/s"), "{text}");
        assert!(text.contains("camera: follow"), "{text}");
        assert!(text.contains("R: throw again"), "{text}");
    }

    #[test]
    fn hud_shows_a_placeholder_until_fps_is_known() {
        let ball = ball(DVec2::ZERO, DVec2::ZERO);
        let sim = Simulation::new(World::new(vec![ball]), FRAME);
        let mut hud = Hud::default();
        hud.update(&sim, 0, 0, &FrameStats::default(), CameraMode::Fixed);
        assert_eq!(hud.lines().next(), Some("-- FPS"));
    }

    #[test]
    fn hud_text_is_replaced_not_appended_each_frame() {
        let ball = ball(DVec2::ZERO, DVec2::ZERO);
        let sim = Simulation::new(World::new(vec![ball]), FRAME);
        let mut hud = Hud::default();
        hud.update(&sim, 0, 0, &FrameStats::default(), CameraMode::Fixed);
        let once = hud.lines().count();
        hud.update(&sim, 0, 0, &FrameStats::default(), CameraMode::Fixed);
        assert_eq!(hud.lines().count(), once);
    }

    #[test]
    fn hidpi_display_uses_twice_the_physical_pixels() {
        assert_eq!(font_size(2.0), 2.0 * font_size(1.0));
        let (x, y) = line_position(3, 1.0);
        assert_eq!(line_position(3, 2.0), (2.0 * x, 2.0 * y));
    }

    #[test]
    fn lines_stack_downwards() {
        let (_, first) = line_position(0, 1.0);
        let (_, second) = line_position(1, 1.0);
        assert!(second > first); // screen y grows downwards
    }
}
