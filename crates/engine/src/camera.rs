//! The camera: what it looks at, from where, and how it follows. Pure
//! logic in world space (NED metres, like the physics), tested without a
//! window; the renderer converts to its own axes and hands the result to
//! the backend.
//!
//! Engines keep all of this in one camera object (Godot's `Camera3D`,
//! Unity's `Camera` plus a follow script). This one follows a target from
//! a fixed direction, south-west and above, at a distance the mouse wheel
//! changes.
//!
//! A good game camera doesn't snap to its target: it *glides* towards it,
//! so the view feels calm. This uses real frame time, not physics time:
//! the camera is presentation, not simulation.
// TODO(step-05): chase camera (behind the target, along its motion) and
// orbit camera (mouse), switched with a key.

use physics::DVec3;

/// Starting distance from the target, in metres.
pub(crate) const DEFAULT_DISTANCE: f64 = 40.0;

/// Distance limits, in metres: the mouse wheel can't put the camera inside
/// the target or out of sight of it.
const MIN_DISTANCE: f64 = 5.0;
const MAX_DISTANCE: f64 = 400.0;

/// Scroll distance (pixels; a mouse-wheel notch counts as 10) that halves
/// the camera distance (zooms in 2x). Exponential, so each notch scales the
/// view by the same ratio at any distance.
const ZOOM_DOUBLING: f64 = 300.0;

/// Time for the camera to close half the gap to its target, in seconds.
/// Smaller = snappier, larger = floatier.
const HALF_LIFE: f64 = 0.15;

/// How far above the horizon the camera looks down from, in degrees.
const ELEVATION_DEGREES: f64 = 25.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CameraMode {
    /// Glides after the target.
    Follow,
    /// Stays where it is.
    Fixed,
}

impl CameraMode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            CameraMode::Follow => "follow",
            CameraMode::Fixed => "fixed",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Camera {
    /// The point the camera looks at (NED, metres). Glides after the
    /// followed object.
    pub(crate) target: DVec3,
    pub(crate) mode: CameraMode,
    /// Distance from the eye to the target, in metres. Private: only
    /// `zoom_by` changes it, so it always stays within the limits.
    distance: f64,
}

impl Camera {
    pub(crate) fn new(target: DVec3) -> Self {
        Self {
            target,
            mode: CameraMode::Follow,
            distance: DEFAULT_DISTANCE,
        }
    }

    /// Where the camera is (NED, metres): `distance` away from the target,
    /// towards the south-west and up.
    pub(crate) fn eye(&self) -> DVec3 {
        self.target + view_direction() * self.distance
    }

    /// Zooms in (positive `scroll`: closer) or out (negative: further),
    /// `scroll` being the scroll distance in pixels.
    pub(crate) fn zoom_by(&mut self, scroll: f64) {
        let factor = 0.5_f64.powf(scroll / ZOOM_DOUBLING);
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub(crate) fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            CameraMode::Follow => CameraMode::Fixed,
            CameraMode::Fixed => CameraMode::Follow,
        };
    }

    /// Glides towards `target` (NED, metres) for one frame lasting
    /// `frame_time` seconds.
    pub(crate) fn update(&mut self, target: DVec3, frame_time: f64) {
        if self.mode == CameraMode::Fixed {
            return;
        }
        self.target += (target - self.target) * smoothing_factor(frame_time);
    }
}

/// Unit vector from the target towards the eye (NED): from the south-west
/// (south = -north, west = -east), `ELEVATION_DEGREES` above the horizon
/// (up = -down).
fn view_direction() -> DVec3 {
    let elevation = ELEVATION_DEGREES.to_radians();
    let horizontal = elevation.cos() * std::f64::consts::FRAC_1_SQRT_2;
    DVec3::new(-horizontal, -horizontal, -elevation.sin())
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

    /// Moves `camera` towards a fixed `target` for `seconds`, in frames of
    /// `frame_time`.
    fn run(mut camera: Camera, target: DVec3, seconds: f64, frame_time: f64) -> Camera {
        let frames = (seconds / frame_time).round() as u32;
        for _ in 0..frames {
            camera.update(target, frame_time);
        }
        camera
    }

    /// A camera on the origin and a target 10 m north of it.
    fn start() -> (Camera, DVec3) {
        (Camera::new(DVec3::ZERO), DVec3::new(10.0, 0.0, 0.0))
    }

    #[test]
    fn starts_at_the_default_distance() {
        assert_eq!(Camera::new(DVec3::ZERO).distance, DEFAULT_DISTANCE);
    }

    #[test]
    fn looks_at_its_target_from_the_south_west_and_above() {
        let target = DVec3::new(100.0, 50.0, -10.0);
        let camera = Camera::new(target);
        let eye = camera.eye();
        // NED: south = less north, west = less east, above = less down.
        assert!(eye.x < target.x && eye.y < target.y && eye.z < target.z);
        assert!(((eye - target).length() - DEFAULT_DISTANCE).abs() < 1e-9);
    }

    #[test]
    fn scrolling_up_moves_closer_and_down_further() {
        let mut camera = Camera::new(DVec3::ZERO);
        camera.zoom_by(ZOOM_DOUBLING);
        assert!((camera.distance - DEFAULT_DISTANCE / 2.0).abs() < 1e-9);
        camera.zoom_by(-ZOOM_DOUBLING);
        // Exponential: in then out by the same amount comes back exactly.
        assert!((camera.distance - DEFAULT_DISTANCE).abs() < 1e-9);
    }

    #[test]
    fn distance_is_kept_within_limits() {
        let mut camera = Camera::new(DVec3::ZERO);
        camera.zoom_by(1e6);
        assert_eq!(camera.distance, MIN_DISTANCE);
        camera.zoom_by(-1e6);
        assert_eq!(camera.distance, MAX_DISTANCE);
    }

    #[test]
    fn the_gap_to_the_target_halves_every_half_life() {
        let (camera, target) = start();
        let camera = run(camera, target, HALF_LIFE, HALF_LIFE / 10.0);
        assert!((camera.target.x - 5.0).abs() < 1e-9, "{}", camera.target.x);
        let camera = run(camera, target, HALF_LIFE, HALF_LIFE / 10.0);
        assert!((camera.target.x - 7.5).abs() < 1e-9, "{}", camera.target.x);
    }

    #[test]
    fn smoothing_does_not_depend_on_the_frame_rate() {
        // Same 1 s of real time at 30, 60 and 144 FPS: same camera target.
        // (A naive `pos += (target - pos) * 0.1` per frame would move
        // almost 5x further at 144 FPS than at 30.)
        let (camera, target) = start();
        let at_30 = run(camera, target, 1.0, 1.0 / 30.0).target;
        let at_60 = run(camera, target, 1.0, 1.0 / 60.0).target;
        let at_144 = run(camera, target, 1.0, 1.0 / 144.0).target;
        assert!((at_30 - at_60).length() < 1e-9);
        assert!((at_60 - at_144).length() < 1e-9);
    }

    #[test]
    fn camera_ends_up_on_the_target() {
        let (camera, target) = start();
        let camera = run(camera, target, 5.0, 1.0 / 60.0);
        assert!((camera.target - target).length() < 1e-6);
    }

    #[test]
    fn fixed_camera_stays_put() {
        let (mut camera, target) = start();
        camera.toggle_mode();
        assert_eq!(camera.mode, CameraMode::Fixed);
        let camera = run(camera, target, 1.0, 1.0 / 60.0);
        assert_eq!(camera.target, DVec3::ZERO);
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
