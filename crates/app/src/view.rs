//! World-space framing: what part of the world the camera shows, and the
//! distance ruler drawn along the ground. Pure functions with no graphics
//! calls, so they can be unit tested without opening a window; `main.rs`
//! does the drawing.
//!
//! Two coordinate spaces are in play, as in every game engine:
//! - **World space**: metres, x right, y up, same as physics. The scene
//!   (ball, ground, ruler) lives here, and the *camera* decides which part
//!   of the world appears on screen, and how big. (This module.)
//! - **Screen space**: pixels, origin top-left, y down. Only the HUD lives
//!   there (see `hud`): it stays put whatever the camera does.
//!
//! Zoom is in *physical* pixels per metre. A HiDPI ("Retina") display has
//! `scale` = 2 physical pixels per logical pixel in each direction, so
//! sizes are designed in logical pixels and multiplied by `scale`.

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
#[derive(Debug)]
pub struct Tick {
    /// Position along the ground, in metres.
    pub x: f64,
    /// How far the mark extends below the ground, in metres.
    pub length: f64,
}

/// One tick per whole metre between `from` and `to` (metres), longer
/// every 10 m. Only the visible part of the ruler is generated.
///
/// Returns an *iterator*: ticks are produced one by one as the caller draws
/// them, never stored, so no memory is allocated per frame (a `Vec` would
/// be allocated and freed every frame).
pub fn ruler_ticks(from: f64, to: f64) -> impl Iterator<Item = Tick> {
    (from.ceil() as i64..=to.floor() as i64).map(|m| Tick {
        x: m as f64,
        // rem_euclid, not %: -10 % 10 is 0 but -3 % 10 is -3 in Rust.
        length: if m.rem_euclid(10) == 0 {
            MAJOR_TICK
        } else {
            MINOR_TICK
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_starts_at_the_same_size_on_any_display() {
        assert_eq!(initial_zoom(2.0), 2.0 * initial_zoom(1.0));
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
        let xs: Vec<f64> = ruler_ticks(0.5, 3.2).map(|t| t.x).collect();
        assert_eq!(xs, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn every_tenth_tick_is_longer_including_negative_ones() {
        let ticks: Vec<Tick> = ruler_ticks(-10.0, 10.0).collect();
        assert_eq!(ticks.len(), 21);
        let long: Vec<f64> = ticks
            .iter()
            .filter(|t| t.length == MAJOR_TICK)
            .map(|t| t.x)
            .collect();
        assert_eq!(long, [-10.0, 0.0, 10.0]);
        const { assert!(MINOR_TICK < MAJOR_TICK) } // checked at compile time
    }
}
