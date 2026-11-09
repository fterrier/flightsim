//! What to show where. Pure functions with no graphics calls, so they can
//! be unit tested without opening a window; `main.rs` does the drawing.
//!
//! Two coordinate spaces are in play, as in every game engine:
//! - **World space**: metres, x right, y up, same as physics. The scene
//!   (ball, ground, ruler) lives here, and the *camera* decides which part
//!   of the world appears on screen, and how big.
//! - **Screen space**: pixels, origin top-left, y down. Only the HUD (the
//!   text overlay) lives here: it stays put whatever the camera does.
//!
//! Screen sizes are *physical* pixels. A HiDPI ("Retina") display has
//! `scale` = 2 physical pixels per logical pixel in each direction, so
//! sizes are designed in logical pixels and multiplied by `scale`.

use crate::timestep::FixedTimestep;
use physics::{DVec2, World};

/// Starting zoom: logical pixels per metre. Scroll to zoom in/out.
const PIXELS_PER_METRE: f32 = 15.0;

/// The camera looks at this height (metres): the ground sits in the lower
/// part of the screen, and a ball starting 20 m up stays below the HUD.
const CAMERA_HEIGHT: f64 = 14.0;

/// Ruler tick lengths, in metres (they're world-space objects).
const MAJOR_TICK: f64 = 1.0;
const MINOR_TICK: f64 = 0.4;

/// Starting camera zoom, in physical pixels per metre.
pub fn initial_zoom(scale: f32) -> f32 {
    PIXELS_PER_METRE * scale
}

/// Where the camera looks: it follows the target horizontally, but stays
/// at a fixed height so the ground remains in view.
// TODO(step-03): follow vertically too (with a dead zone), and glide
// towards the target with frame-rate-independent smoothing instead of
// snapping to it.
pub fn camera_target(target: DVec2) -> DVec2 {
    DVec2::new(target.x, CAMERA_HEIGHT)
}

/// The world x range visible on screen, in metres, for a camera centred on
/// `centre_x` (metres) with `zoom` (pixels per metre) on a screen
/// `screen_width` pixels wide.
pub fn visible_x_range(centre_x: f64, screen_width: f32, zoom: f32) -> (f64, f64) {
    let half_width = (screen_width / zoom / 2.0) as f64;
    (centre_x - half_width, centre_x + half_width)
}

/// One mark of the distance ruler drawn along the ground (world space).
pub struct Tick {
    /// Position along the ground, in metres.
    pub x: f64,
    /// How far the mark extends below the ground, in metres.
    pub length: f64,
}

/// One tick per whole metre between `from` and `to` (metres), longer
/// every 10 m. Only the visible part of the ruler is generated.
pub fn ruler_ticks(from: f64, to: f64) -> Vec<Tick> {
    (from.ceil() as i64..=to.floor() as i64)
        .map(|m| Tick {
            x: m as f64,
            // rem_euclid, not %: -10 % 10 is 0 but -3 % 10 is -3 in Rust.
            length: if m.rem_euclid(10) == 0 {
                MAJOR_TICK
            } else {
                MINOR_TICK
            },
        })
        .collect()
}

/// HUD text size, in physical pixels.
pub fn hud_font_size(scale: f32) -> f32 {
    22.0 * scale
}

/// Screen position (physical pixels) of HUD text line number `line`.
pub fn hud_line_position(line: usize, scale: f32) -> (f32, f32) {
    (12.0 * scale, (12.0 + 26.0 * line as f32) * scale)
}

/// HUD text lines, top to bottom.
///
/// Watch "sim time" vs "real time": they stay equal at any frame rate,
/// until frames exceed `MAX_FRAME_TIME` and the simulation falls behind.
///
/// `frame_time` is the duration of the last frame, in seconds.
pub fn overlay_lines(
    world: &World,
    clock: &FixedTimestep,
    steps: u32,
    frame_time: f64,
) -> Vec<String> {
    let fps = if frame_time > 0.0 {
        format!("{:.0}", 1.0 / frame_time)
    } else {
        "--".to_owned()
    };
    let ball = &world.ball;
    vec![
        format!("{fps} FPS"),
        format!("{steps} physics steps this frame"),
        format!("sim time  {:5.2} s", world.time),
        format!("real time {:5.2} s", clock.real_time()),
        format!("x = {:.1} m", ball.position.x),
        // Height of the bottom of the ball, so it reads 0 when resting.
        format!("height = {:.1} m", ball.position.y - world.ball_radius),
        format!("vertical speed = {:.1} m/s", ball.velocity.y),
        "R: throw again | hold L: slow frames | hold K: stalled frames | scroll: zoom | Esc: quit"
            .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::Body;

    #[test]
    fn camera_follows_the_target_horizontally_at_a_fixed_height() {
        let target = camera_target(DVec2::new(123.0, 2.0));
        assert_eq!(target, DVec2::new(123.0, CAMERA_HEIGHT));
    }

    #[test]
    fn visible_range_is_centred_on_the_camera() {
        // 400 px wide at 20 px/m = 20 m across, centred on x = 100 m.
        assert_eq!(visible_x_range(100.0, 400.0, 20.0), (90.0, 110.0));
    }

    #[test]
    fn ruler_has_a_tick_on_every_whole_metre_in_range() {
        let xs: Vec<f64> = ruler_ticks(0.5, 3.2).iter().map(|t| t.x).collect();
        assert_eq!(xs, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn every_tenth_tick_is_longer_including_negative_ones() {
        let ticks = ruler_ticks(-10.0, 10.0);
        assert_eq!(ticks.len(), 21);
        let long: Vec<f64> = ticks
            .iter()
            .filter(|t| t.length == MAJOR_TICK)
            .map(|t| t.x)
            .collect();
        assert_eq!(long, [-10.0, 0.0, 10.0]);
        const { assert!(MINOR_TICK < MAJOR_TICK) } // checked at compile time
    }

    #[test]
    fn hidpi_display_uses_twice_the_physical_pixels() {
        assert_eq!(initial_zoom(2.0), 2.0 * initial_zoom(1.0));
        assert_eq!(hud_font_size(2.0), 2.0 * hud_font_size(1.0));
        let (x, y) = hud_line_position(3, 1.0);
        assert_eq!(hud_line_position(3, 2.0), (2.0 * x, 2.0 * y));
    }

    #[test]
    fn hud_lines_stack_downwards() {
        let (_, first) = hud_line_position(0, 1.0);
        let (_, second) = hud_line_position(1, 1.0);
        assert!(second > first); // screen y grows downwards
    }

    #[test]
    fn overlay_shows_timing_position_and_speed() {
        // A ball resting on the ground (radius 0.4 m), sliding at 10 m/s.
        let mut world = World::new(
            Body {
                position: DVec2::new(0.0, 0.4),
                velocity: DVec2::new(10.0, 0.0),
            },
            0.4,
        );
        let mut clock = FixedTimestep::new(1.0 / 64.0);
        // A 0.25 s frame (the longest accepted without capping) = 16 steps.
        let steps = clock.advance(0.25);
        for _ in 0..steps {
            world.step(clock.dt());
        }

        let text = overlay_lines(&world, &clock, steps, 1.0 / 60.0).join("\n");
        assert!(text.contains("60 FPS"), "{text}");
        assert!(text.contains("16 physics steps this frame"), "{text}");
        assert!(text.contains("sim time   0.25 s"), "{text}");
        assert!(text.contains("real time  0.25 s"), "{text}");
        assert!(text.contains("x = 2.5 m"), "{text}");
        assert!(text.contains("height = 0.0 m"), "{text}"); // above ground
        assert!(text.contains("vertical speed = 0.0 m/s"), "{text}");
        assert!(text.contains("R: throw again"), "{text}");
    }

    #[test]
    fn overlay_has_no_fps_before_the_first_frame_is_timed() {
        let world = World::new(
            Body {
                position: DVec2::ZERO,
                velocity: DVec2::ZERO,
            },
            0.4,
        );
        let clock = FixedTimestep::new(1.0 / 64.0);
        // A zero frame time would mean "infinite FPS": show a placeholder.
        let text = overlay_lines(&world, &clock, 0, 0.0).join("\n");
        assert!(text.contains("-- FPS"), "{text}");
    }
}
