//! Camera behaviour: where the camera looks, and how it gets there.
//!
//! Pure logic in world space (metres), tested without a window. `main.rs`
//! copies the result into kiss3d's camera, which does the projection.
//!
//! A good game camera doesn't snap to its target: it *glides* towards it,
//! and doesn't move at all for small motions (a dead zone), so the view
//! feels calm. This uses real frame time, not physics time: the camera is
//! presentation, not simulation.
//!
//! Framing is expressed as fractions of what's visible (`half_height`:
//! metres visible above and below the centre), not in metres, so it keeps
//! working when the player zooms.

use physics::DVec2;

/// Time for the camera to close half the gap to where it wants to be, in
/// seconds. Smaller = snappier, larger = floatier.
const HALF_LIFE: f64 = 0.15;

/// The target can move this far above or below the camera centre before
/// the camera follows vertically, as a fraction of the visible half-height
/// (0.2: the target may roam the middle 20% of the screen height).
const DEAD_ZONE: f64 = 0.2;

/// The camera centre never goes lower than this, as a fraction of the
/// visible half-height: 0.5 puts the ground at most 25% up from the bottom
/// of the screen.
const MIN_HEIGHT: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraMode {
    /// Glides after the target.
    Follow,
    /// Stays where it is.
    Fixed,
}

impl CameraMode {
    pub fn name(self) -> &'static str {
        match self {
            CameraMode::Follow => "follow",
            CameraMode::Fixed => "fixed",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FollowCamera {
    /// The point the camera looks at (centre of the screen), in metres.
    pub position: DVec2,
    pub mode: CameraMode,
}

impl FollowCamera {
    pub fn new(position: DVec2) -> Self {
        Self {
            position,
            mode: CameraMode::Follow,
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            CameraMode::Follow => CameraMode::Fixed,
            CameraMode::Fixed => CameraMode::Follow,
        };
    }

    /// Where the camera would like to be to frame `target` (metres), with
    /// `half_height` metres visible above and below the centre: centred on
    /// the target horizontally; vertically, moved only as far as needed to
    /// keep the target inside the dead zone, and never so low that the
    /// ground rises into the upper part of the screen.
    pub fn desired_position(&self, target: DVec2, half_height: f64) -> DVec2 {
        let zone = DEAD_ZONE * half_height;
        let y = target
            .y
            .clamp(self.position.y - zone, self.position.y + zone);
        // `y` is where the target would sit if the camera didn't move;
        // shifting the camera by (target - y) brings it back to the edge
        // of the dead zone.
        let camera_y = self.position.y + (target.y - y);
        DVec2::new(target.x, camera_y.max(MIN_HEIGHT * half_height))
    }

    /// Frames `target` immediately, without gliding (at startup, or after
    /// a teleport where gliding across the world would look odd).
    pub fn snap_to(&mut self, target: DVec2, half_height: f64) {
        self.position = DVec2::new(target.x, target.y.max(MIN_HEIGHT * half_height));
    }

    /// Moves the camera towards `target` for one frame lasting
    /// `frame_time` seconds.
    pub fn update(&mut self, target: DVec2, frame_time: f64, half_height: f64) {
        if self.mode == CameraMode::Fixed {
            return;
        }
        let desired = self.desired_position(target, half_height);
        self.position += (desired - self.position) * smoothing_factor(frame_time);
    }
}

/// Fraction of the remaining gap to close in a frame of `frame_time`
/// seconds, so that the gap halves every `HALF_LIFE` seconds *whatever the
/// frame rate*.
///
/// The naive `position += gap * 0.1` per frame closes 10% per frame: twice
/// as fast at 120 FPS as at 60. Exponential decay fixes it: over a time t
/// the remaining gap is multiplied by 0.5^(t / HALF_LIFE), and doing that
/// in two half-frames gives exactly the same result as in one full frame.
fn smoothing_factor(frame_time: f64) -> f64 {
    1.0 - 0.5_f64.powf(frame_time / HALF_LIFE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Visible half-height used in most tests: 20 m above and below the
    /// camera centre.
    const H: f64 = 20.0;
    /// Lowest camera height at that zoom.
    const LOWEST: f64 = MIN_HEIGHT * H;
    /// Dead zone half-height at that zoom.
    const ZONE: f64 = DEAD_ZONE * H;

    /// Moves `camera` towards a fixed `target` for `seconds`, in frames of
    /// `frame_time`.
    fn run(mut camera: FollowCamera, target: DVec2, seconds: f64, frame_time: f64) -> FollowCamera {
        let frames = (seconds / frame_time).round() as u32;
        for _ in 0..frames {
            camera.update(target, frame_time, H);
        }
        camera
    }

    /// A camera at its lowest height and a target 10 m to the right at the
    /// same height (inside the dead zone, so only x moves).
    fn start() -> (FollowCamera, DVec2) {
        (
            FollowCamera::new(DVec2::new(0.0, LOWEST)),
            DVec2::new(10.0, LOWEST),
        )
    }

    #[test]
    fn the_gap_to_the_target_halves_every_half_life() {
        let (camera, target) = start();
        let camera = run(camera, target, HALF_LIFE, HALF_LIFE / 10.0);
        assert!(
            (camera.position.x - 5.0).abs() < 1e-9,
            "{}",
            camera.position.x
        );
        let camera = run(camera, target, HALF_LIFE, HALF_LIFE / 10.0);
        assert!(
            (camera.position.x - 7.5).abs() < 1e-9,
            "{}",
            camera.position.x
        );
    }

    #[test]
    fn smoothing_does_not_depend_on_the_frame_rate() {
        // Same 1 s of real time at 30, 60 and 144 FPS: same camera position.
        // (A naive `pos += (target - pos) * 0.1` per frame would move
        // almost 5x further at 144 FPS than at 30.)
        let (camera, target) = start();
        let at_30 = run(camera, target, 1.0, 1.0 / 30.0).position;
        let at_60 = run(camera, target, 1.0, 1.0 / 60.0).position;
        let at_144 = run(camera, target, 1.0, 1.0 / 144.0).position;
        assert!((at_30 - at_60).length() < 1e-9);
        assert!((at_60 - at_144).length() < 1e-9);
    }

    #[test]
    fn camera_ends_up_on_the_target() {
        let (camera, target) = start();
        let camera = run(camera, target, 5.0, 1.0 / 60.0);
        assert!((camera.position - target).length() < 1e-6);
    }

    #[test]
    fn no_vertical_movement_while_the_target_is_inside_the_dead_zone() {
        let camera = FollowCamera::new(DVec2::new(0.0, 50.0));
        let target = DVec2::new(0.0, 50.0 + ZONE * 0.9);
        assert_eq!(camera.desired_position(target, H).y, 50.0);
    }

    #[test]
    fn camera_moves_just_enough_to_keep_the_target_in_the_dead_zone() {
        let camera = FollowCamera::new(DVec2::new(0.0, 50.0));
        let above = DVec2::new(0.0, 50.0 + ZONE + 3.0);
        assert_eq!(camera.desired_position(above, H).y, 53.0);
        let below = DVec2::new(0.0, 50.0 - ZONE - 3.0);
        assert_eq!(camera.desired_position(below, H).y, 47.0);
    }

    #[test]
    fn ground_stays_in_the_lower_part_of_the_screen_at_any_zoom() {
        // Framing is a fraction of what's visible, not a fixed height in
        // metres: zoomed in 10x, the camera may go 10x lower.
        let on_ground = DVec2::new(0.0, 0.0);
        for half_height in [2.0, 20.0, 200.0] {
            let camera = FollowCamera::new(DVec2::new(0.0, half_height));
            let y = camera.desired_position(on_ground, half_height).y;
            // Ground (y = 0) is `y` below the centre: within the visible
            // half-height, and in the lower half of the screen.
            assert_eq!(y, MIN_HEIGHT * half_height);
            assert!(y > 0.0 && y < half_height);
        }
    }

    #[test]
    fn snap_frames_the_target_immediately() {
        let mut camera = FollowCamera::new(DVec2::ZERO);
        camera.snap_to(DVec2::new(50.0, 30.0), H);
        assert_eq!(camera.position, DVec2::new(50.0, 30.0));
        camera.snap_to(DVec2::new(0.0, 0.0), H); // on the ground
        assert_eq!(camera.position, DVec2::new(0.0, LOWEST));
    }

    #[test]
    fn fixed_camera_stays_put() {
        let (mut camera, target) = start();
        camera.toggle_mode();
        assert_eq!(camera.mode, CameraMode::Fixed);
        let camera = run(camera, target, 1.0, 1.0 / 60.0);
        assert_eq!(camera.position, DVec2::new(0.0, LOWEST));
    }

    #[test]
    fn toggling_twice_goes_back_to_following() {
        let (mut camera, _) = start();
        assert_eq!(camera.mode, CameraMode::Follow);
        camera.toggle_mode();
        camera.toggle_mode();
        assert_eq!(camera.mode, CameraMode::Follow);
    }

    #[test]
    fn modes_have_display_names() {
        assert_eq!(CameraMode::Follow.name(), "follow");
        assert_eq!(CameraMode::Fixed.name(), "fixed");
    }
}
