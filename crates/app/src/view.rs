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

use crate::camera::CameraMode;
use crate::timestep::FixedTimestep;
use physics::{DVec2, World};

/// Starting zoom: logical pixels per metre. Scroll to zoom in/out.
const PIXELS_PER_METRE: f32 = 15.0;

/// Zoom limits, in logical pixels per metre.
const MIN_ZOOM: f32 = 3.0;
const MAX_ZOOM: f32 = 60.0;

/// Ruler tick lengths, in metres (they're world-space objects).
const MAJOR_TICK: f64 = 1.0;
const MINOR_TICK: f64 = 0.4;

/// Starting camera zoom, in physical pixels per metre.
pub fn initial_zoom(scale: f32) -> f32 {
    PIXELS_PER_METRE * scale
}

/// Keeps a camera zoom (physical pixels per metre) within the limits, so
/// the mouse wheel can't zoom into a blur or out to nothing.
pub fn clamp_zoom(zoom: f32, scale: f32) -> f32 {
    zoom.clamp(MIN_ZOOM * scale, MAX_ZOOM * scale)
}

/// The part of a physics state that rendering needs: only what is drawn.
///
/// Used for interpolation. Before each physics step the game loop captures
/// one of these, so the renderer can draw *between* the previous and the
/// current state. Engines do the same, keeping the previous transform of
/// each interpolated object only (Bevy's `PreviousPhysicalTranslation`,
/// Godot's per-object previous transform, Unity's Rigidbody
/// interpolation), not a copy of the whole world: velocities, masses or
/// sim time aren't drawn, so they aren't copied. It's 16 bytes, on the
/// stack, never a heap allocation.
// TODO(step-04): positions become `DVec3`.
// TODO(step-08): add orientations (quaternions), blended with `slerp`.
// TODO(step-11): one entry per *moving* entity, in a contiguous array;
// static objects (buildings) need none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderState {
    /// Ball centre, in metres.
    pub ball: DVec2,
}

impl RenderState {
    pub fn capture(world: &World) -> Self {
        Self {
            ball: world.ball.position,
        }
    }

    /// The state to draw, `alpha` of the way (0 to 1) from `self` (the
    /// previous state) to `current`.
    ///
    /// Trade-off: what you see is always up to one physics step (8 ms at
    /// 120 Hz) behind the simulation. Smooth beats a few milliseconds here;
    /// the alternative, *extrapolating* ahead, overshoots whenever something
    /// changes direction (like the ball landing).
    pub fn interpolate(&self, current: &Self, alpha: f64) -> Self {
        Self {
            ball: self.ball.lerp(current.ball, alpha),
        }
    }
}

/// The world x range visible on screen, in metres, for a camera centred on
/// `centre_x` (metres) with `zoom` (pixels per metre) on a screen
/// `screen_width` pixels wide.
pub fn visible_x_range(centre_x: f64, screen_width: f32, zoom: f32) -> (f64, f64) {
    let half_width = (screen_width / zoom / 2.0) as f64;
    (centre_x - half_width, centre_x + half_width)
}

/// Metres visible above (and below) the camera centre, for a screen
/// `screen_height` pixels tall at `zoom` pixels per metre.
pub fn visible_half_height(screen_height: f32, zoom: f32) -> f64 {
    (screen_height / zoom / 2.0) as f64
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
    18.0 * scale
}

/// Screen position (physical pixels) of HUD text line number `line`.
pub fn hud_line_position(line: usize, scale: f32) -> (f32, f32) {
    (12.0 * scale, (12.0 + 21.0 * line as f32) * scale)
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
    camera_mode: CameraMode,
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
        format!(
            "drawn {:.0}% of the way to the next step",
            clock.alpha() * 100.0
        ),
        format!("sim time  {:5.2} s", world.time),
        format!("real time {:5.2} s", clock.real_time()),
        format!("x = {:.1} m", ball.position.x),
        // Height of the bottom of the ball, so it reads 0 when resting.
        format!("height = {:.1} m", ball.position.y - world.ball_radius),
        format!("vertical speed = {:.1} m/s", ball.velocity.y),
        format!("camera: {} (C to switch)", camera_mode.name()),
        "R: throw again | hold L: slow frames | hold K: stalled frames | scroll: zoom | Esc: quit"
            .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::Body;

    #[test]
    fn render_state_captures_only_what_is_drawn() {
        // The doc comment promises 16 bytes; checked at compile time.
        const { assert!(std::mem::size_of::<RenderState>() == 16) }
        let world = World::new(
            Body {
                position: DVec2::new(3.0, 4.0),
                velocity: DVec2::new(5.0, 6.0),
            },
            0.4,
        );
        assert_eq!(
            RenderState::capture(&world),
            RenderState {
                ball: DVec2::new(3.0, 4.0)
            }
        );
    }

    #[test]
    fn interpolation_blends_the_last_two_physics_states() {
        let previous = RenderState {
            ball: DVec2::new(0.0, 10.0),
        };
        let current = RenderState {
            ball: DVec2::new(4.0, 6.0),
        };
        assert_eq!(previous.interpolate(&current, 0.0), previous);
        assert_eq!(
            previous.interpolate(&current, 0.25).ball,
            DVec2::new(1.0, 9.0)
        );
        assert_eq!(previous.interpolate(&current, 1.0), current);
    }

    #[test]
    fn zoom_is_kept_within_limits() {
        assert_eq!(clamp_zoom(1000.0, 1.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(0.01, 1.0), MIN_ZOOM);
        assert_eq!(clamp_zoom(20.0, 1.0), 20.0);
        // Limits are in logical pixels per metre: doubled on Retina.
        assert_eq!(clamp_zoom(1000.0, 2.0), 2.0 * MAX_ZOOM);
    }

    #[test]
    fn visible_half_height_depends_on_screen_and_zoom() {
        // 600 px tall at 20 px/m = 30 m tall = 15 m above and below centre.
        assert_eq!(visible_half_height(600.0, 20.0), 15.0);
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

        let text = overlay_lines(&world, &clock, steps, 1.0 / 60.0, CameraMode::Follow).join("\n");
        assert!(text.contains("60 FPS"), "{text}");
        assert!(text.contains("16 physics steps this frame"), "{text}");
        assert!(text.contains("sim time   0.25 s"), "{text}");
        assert!(text.contains("real time  0.25 s"), "{text}");
        assert!(text.contains("x = 2.5 m"), "{text}");
        assert!(text.contains("height = 0.0 m"), "{text}"); // above ground
        assert!(text.contains("vertical speed = 0.0 m/s"), "{text}");
        assert!(text.contains("R: throw again"), "{text}");
        // 0.25 s = exactly 16 steps, so nothing left over: drawn exactly
        // at the latest physics state.
        assert!(
            text.contains("drawn 0% of the way to the next step"),
            "{text}"
        );
        assert!(text.contains("camera: follow (C to switch)"), "{text}");
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
        let text = overlay_lines(&world, &clock, 0, 0.0, CameraMode::Fixed).join("\n");
        assert!(text.contains("-- FPS"), "{text}");
    }
}
