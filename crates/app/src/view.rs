//! What to draw where: converts simulation coordinates (metres, y up) into
//! screen pixels (y down) and prepares the text overlay.
//!
//! Pure functions with no graphics calls, so they can be unit tested
//! without opening a window. `main.rs` does the actual drawing.

use crate::timestep::FixedTimestep;
use physics::{DVec2, World};

/// Zoom level: how many screen pixels one metre takes.
const PIXELS_PER_METRE: f32 = 20.0;

/// Maps the world onto the screen. Rebuilt every frame, so window resizes
/// are picked up automatically.
// TODO(step-03): replace the wrap-around with a camera that follows the
// plane (the view gets a centre position in metres).
pub struct View {
    pub pixels_per_metre: f32,
    pub screen_width: f32,
    /// Screen y of the ground line (y = 0 m), in pixels.
    pub ground_y: f32,
}

/// One mark of the distance ruler drawn along the ground.
pub struct Tick {
    /// Screen x, in pixels.
    pub x: f32,
    /// Length of the mark, in pixels.
    pub length: f32,
    /// Distance label, only on every 10th metre.
    pub label: Option<String>,
}

impl View {
    pub fn for_screen(width: f32, height: f32) -> Self {
        Self {
            pixels_per_metre: PIXELS_PER_METRE,
            screen_width: width,
            ground_y: height * 0.6,
        }
    }

    /// Screen position (pixels) of a world position (metres).
    ///
    /// x wraps around the screen width so a moving object stays visible.
    pub fn to_screen(&self, position: DVec2) -> (f32, f32) {
        // Wrap in f64 *before* converting: f32 loses sub-pixel precision
        // once positions reach a few kilometres.
        let x = (position.x * self.pixels_per_metre as f64).rem_euclid(self.screen_width as f64);
        let y = self.ground_y - position.y as f32 * self.pixels_per_metre;
        (x as f32, y)
    }

    /// One tick per metre across the screen, longer and labelled every 10 m.
    pub fn ruler_ticks(&self) -> Vec<Tick> {
        let metres_across = (self.screen_width / self.pixels_per_metre) as u32;
        (0..=metres_across)
            .map(|m| {
                let major = m % 10 == 0;
                Tick {
                    x: m as f32 * self.pixels_per_metre,
                    length: if major { 12.0 } else { 5.0 },
                    label: major.then(|| format!("{m} m")),
                }
            })
            .collect()
    }
}

/// Debug text shown in the top-left corner.
///
/// Watch "sim time" vs "real time": they stay equal at any frame rate,
/// until frames exceed `MAX_FRAME_TIME` and the simulation falls behind.
pub fn overlay_lines(world: &World, clock: &FixedTimestep, steps: u32, fps: i32) -> Vec<String> {
    vec![
        format!("{fps} FPS"),
        format!("{steps} physics steps this frame"),
        format!("sim time  {:5.2} s", world.time),
        format!("real time {:5.2} s", clock.real_time()),
        format!("x = {:.1} m", world.dot.position.x),
        "hold L: slow frames | hold K: stalled frames | Esc: quit".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::Body;

    /// A 400 x 300 px screen at 20 px/m: 20 m wide, ground at y = 180 px.
    fn view() -> View {
        View::for_screen(400.0, 300.0)
    }

    #[test]
    fn ground_is_at_sixty_percent_of_screen_height() {
        assert_eq!(view().ground_y, 180.0);
    }

    #[test]
    fn origin_is_on_the_ground_at_the_left_edge() {
        assert_eq!(view().to_screen(DVec2::ZERO), (0.0, 180.0));
    }

    #[test]
    fn world_y_up_becomes_screen_y_down() {
        // 1 m above ground = 20 px *higher* on screen = smaller y.
        assert_eq!(view().to_screen(DVec2::new(0.0, 1.0)), (0.0, 160.0));
    }

    #[test]
    fn x_wraps_around_the_screen_width() {
        // The screen is 20 m wide, so 21 m lands where 1 m would.
        assert_eq!(view().to_screen(DVec2::new(21.0, 0.0)), (20.0, 180.0));
        // Negative x wraps to the right side.
        assert_eq!(view().to_screen(DVec2::new(-1.0, 0.0)), (380.0, 180.0));
    }

    #[test]
    fn ruler_has_a_tick_per_metre_and_labels_every_ten() {
        let ticks = view().ruler_ticks();
        assert_eq!(ticks.len(), 21); // 0 m ..= 20 m
        assert_eq!(ticks[0].label.as_deref(), Some("0 m"));
        assert_eq!(ticks[5].x, 100.0);
        assert_eq!(ticks[5].label, None);
        assert!(ticks[5].length < ticks[10].length);
        assert_eq!(ticks[10].label.as_deref(), Some("10 m"));
    }

    #[test]
    fn overlay_shows_timing_and_position() {
        let mut world = World::new(Body {
            position: DVec2::new(0.0, 2.0),
            velocity: DVec2::new(10.0, 0.0),
        });
        let mut clock = FixedTimestep::new(1.0 / 64.0);
        // A 0.25 s frame (the longest accepted without capping) = 16 steps.
        let steps = clock.advance(0.25);
        for _ in 0..steps {
            world.step(clock.dt());
        }

        let text = overlay_lines(&world, &clock, steps, 60).join("\n");
        assert!(text.contains("60 FPS"), "{text}");
        assert!(text.contains("16 physics steps this frame"), "{text}");
        assert!(text.contains("sim time   0.25 s"), "{text}");
        assert!(text.contains("real time  0.25 s"), "{text}");
        assert!(text.contains("x = 2.5 m"), "{text}");
    }
}
