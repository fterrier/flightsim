//! The game's camera: what it looks at, from where, how it follows, and
//! how the player moves it. Game code, as camera scripts are in Unity or
//! Godot: the engine only draws from the `View` it gets each frame. Pure
//! logic in world space (NED metres, like the physics), tested without a
//! window.
//!
//! Like Unreal's spring arm (`USpringArmComponent`) or Cinemachine's
//! orbital follow, the camera hangs off its target on an *arm*: a
//! direction, in spherical coordinates (yaw: compass direction, pitch:
//! angle above the horizon), and a length (distance). Storing the arm
//! rather than the eye position makes each control simple: the mouse
//! wheel changes the length, a mouse drag the angles (orbit), the target's
//! motion the yaw (chase).
//!
//! A good game camera doesn't snap: it *glides*, so the view feels calm.
//! This uses real frame time, not physics time: the camera is
//! presentation, not simulation.
// TODO(step-08): a cockpit camera, and a chase camera behind the aircraft's
// nose (its orientation) rather than its direction of motion.
// TODO(step-12): keep the arm out of buildings and terrain, as Unreal's
// spring arm does by probing for collisions; for now, only the pitch limits
// keep the eye above the target.

use std::f64::consts::{PI, TAU};

use engine::{DVec2, DVec3};

/// Starting distance from the target, in metres.
const DEFAULT_DISTANCE: f64 = 40.0;

/// Distance limits, in metres: the mouse wheel can't put the camera inside
/// the target or out of sight of it.
const MIN_DISTANCE: f64 = 5.0;
const MAX_DISTANCE: f64 = 400.0;

/// Scroll distance (pixels; a mouse-wheel notch counts as 10) that halves
/// the camera distance (zooms in 2x). Exponential, so each notch scales the
/// view by the same ratio at any distance.
const ZOOM_DOUBLING: f64 = 300.0;

/// Starting arm direction: from the south-west (compass 225°), 25° above
/// the horizon.
const START_YAW_DEGREES: f64 = 225.0;
const START_PITCH_DEGREES: f64 = 25.0;

/// Pitch limits, in degrees. Never straight down: there the view direction
/// and the "up" direction the view is built with line up, and the picture
/// flips or spins. Never below the horizon: the eye would sink under the
/// target's ground.
const MIN_PITCH_DEGREES: f64 = 5.0;
const MAX_PITCH_DEGREES: f64 = 85.0;

/// How far a mouse drag turns the arm: radians per (logical) pixel. A
/// 630-pixel drag turns it half a turn, whatever the screen's density.
const ORBIT_RADIANS_PER_PIXEL: f64 = 0.005;

/// Time for the camera to close half the gap to its target, in seconds.
/// Smaller = snappier, larger = floatier.
const HALF_LIFE: f64 = 0.15;

/// The same for the chase camera swinging round behind its target: slower,
/// so a quick change of direction doesn't whip the view around (Unreal's
/// "rotation lag").
const CHASE_HALF_LIFE: f64 = 0.5;

/// Below this horizontal speed (m/s) the direction of motion is mostly
/// noise, and at rest it doesn't exist: the chase camera keeps its heading.
const MIN_CHASE_SPEED: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CameraMode {
    /// Glides after the target; the mouse turns the arm.
    Orbit,
    /// Glides after the target and swings round behind it, along its
    /// direction of motion. The mouse can still turn the arm: the camera
    /// then swings back.
    Chase,
    /// Stays where it is; the mouse can still turn the arm.
    Fixed,
}

impl CameraMode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            CameraMode::Orbit => "orbit",
            CameraMode::Chase => "chase",
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
    /// Compass direction from the target to the eye, in radians: 0 north,
    /// π/2 east. Always in [-π, π).
    yaw: f64,
    /// Angle of the arm above the horizon, in radians, within the limits.
    pitch: f64,
    /// Length of the arm, in metres, within the limits.
    distance: f64,
}

impl Camera {
    pub(crate) fn new(target: DVec3) -> Self {
        Self {
            target,
            mode: CameraMode::Orbit,
            yaw: wrap_angle(START_YAW_DEGREES.to_radians()),
            pitch: START_PITCH_DEGREES.to_radians(),
            distance: DEFAULT_DISTANCE,
        }
    }

    /// Where the camera is (NED, metres): at the end of the arm.
    pub(crate) fn eye(&self) -> DVec3 {
        self.target + arm_direction(self.yaw, self.pitch) * self.distance
    }

    /// Zooms in (positive `scroll`: closer) or out (negative: further),
    /// `scroll` being the scroll distance in pixels.
    pub(crate) fn zoom_by(&mut self, scroll: f64) {
        let factor = 0.5_f64.powf(scroll / ZOOM_DOUBLING);
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Turns the arm by a mouse drag of `drag` logical pixels (x right,
    /// y down). As if grabbing the world: dragging right swings the camera
    /// to the left so the world turns right, under the hand; dragging down
    /// raises it, to look down more.
    pub(crate) fn orbit(&mut self, drag: DVec2) {
        self.yaw = wrap_angle(self.yaw + drag.x * ORBIT_RADIANS_PER_PIXEL);
        self.pitch = (self.pitch + drag.y * ORBIT_RADIANS_PER_PIXEL).clamp(
            MIN_PITCH_DEGREES.to_radians(),
            MAX_PITCH_DEGREES.to_radians(),
        );
    }

    /// Orbit, then chase, then fixed, then orbit again.
    pub(crate) fn next_mode(&mut self) {
        self.mode = match self.mode {
            CameraMode::Orbit => CameraMode::Chase,
            CameraMode::Chase => CameraMode::Fixed,
            CameraMode::Fixed => CameraMode::Orbit,
        };
    }

    /// One frame lasting `frame_time` seconds: glides towards `target`
    /// (NED, metres), and when chasing, swings round behind it according to
    /// its `velocity` (NED, m/s).
    pub(crate) fn update(&mut self, target: DVec3, velocity: DVec3, frame_time: f64) {
        if self.mode == CameraMode::Fixed {
            return;
        }
        self.target += (target - self.target) * smoothing_factor(frame_time, HALF_LIFE);

        // Only the horizontal motion: a ball falling straight down would
        // otherwise swing the camera overhead.
        let horizontal = velocity.truncate();
        if self.mode == CameraMode::Chase && horizontal.length() >= MIN_CHASE_SPEED {
            // Behind = the compass direction of motion, turned round.
            let heading = horizontal.y.atan2(horizontal.x);
            let behind = wrap_angle(heading + PI);
            // Turn the short way round: from 170° to -170° is +20°, not
            // -340°.
            let turn = wrap_angle(behind - self.yaw);
            let k = smoothing_factor(frame_time, CHASE_HALF_LIFE);
            self.yaw = wrap_angle(self.yaw + turn * k);
        }
    }
}

/// Unit vector along an arm with compass direction `yaw` and `pitch` above
/// the horizon (radians), in NED: the usual spherical coordinates, with
/// "up" being -z.
fn arm_direction(yaw: f64, pitch: f64) -> DVec3 {
    DVec3::new(
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        -pitch.sin(),
    )
}

/// `angle` (radians) brought into [-π, π): the same direction, one way
/// to write it. Without this, differences between angles can't be trusted:
/// 350° - 10° = 340°, when the two are 20° apart.
fn wrap_angle(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Fraction of the remaining gap to close in a frame of `frame_time`
/// seconds, so that the gap halves every `half_life` seconds *whatever the
/// frame rate*.
///
/// The naive `position += gap * 0.1` per frame closes 10% per frame: twice
/// as fast at 120 FPS as at 60. Exponential decay fixes it: over a time t
/// the remaining gap is multiplied by 0.5^(t / half_life), and doing that
/// in two half-frames gives exactly the same result as in one full frame.
fn smoothing_factor(frame_time: f64, half_life: f64) -> f64 {
    1.0 - 0.5_f64.powf(frame_time / half_life)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the camera settles, compared within 1e-4 rad (0.006°):
    /// smoothing closes the gap exponentially and never quite arrives.
    /// After 10 s of chase (20 half-lives) a 90° gap is down to 2^-20 of
    /// itself, 1.5e-6 rad.
    const ANGLE_TOLERANCE: f64 = 1e-4;

    /// Results that must be the same whichever way they're computed (frame
    /// rates), compared within rounding error.
    const ROUNDING: f64 = 1e-9;

    /// Runs `camera` for `seconds`, in frames of `frame_time`, with its
    /// followed object at `target` moving at `velocity`.
    fn run(
        mut camera: Camera,
        target: DVec3,
        velocity: DVec3,
        seconds: f64,
        frame_time: f64,
    ) -> Camera {
        let frames = (seconds / frame_time).round() as u32;
        for _ in 0..frames {
            camera.update(target, velocity, frame_time);
        }
        camera
    }

    /// A camera on the origin and a target 10 m north of it, at rest.
    fn start() -> (Camera, DVec3) {
        (Camera::new(DVec3::ZERO), DVec3::new(10.0, 0.0, 0.0))
    }

    /// A chase camera on the origin, its yaw at `yaw_degrees`.
    fn chasing(yaw_degrees: f64) -> Camera {
        let mut camera = Camera::new(DVec3::ZERO);
        camera.mode = CameraMode::Chase;
        camera.yaw = wrap_angle(yaw_degrees.to_radians());
        camera
    }

    /// `v` metres per second along compass direction `degrees`.
    fn moving_towards(degrees: f64, v: f64) -> DVec3 {
        let heading = degrees.to_radians();
        DVec3::new(heading.cos(), heading.sin(), 0.0) * v
    }

    fn assert_angle(actual: f64, expected_degrees: f64) {
        let expected = wrap_angle(expected_degrees.to_radians());
        let gap = wrap_angle(actual - expected).abs();
        assert!(
            gap < ANGLE_TOLERANCE,
            "{}° instead of {expected_degrees}°",
            actual.to_degrees()
        );
    }

    #[test]
    fn the_arm_points_by_compass_direction_and_elevation() {
        // NED: x north, y east, z down (so "up" is -z).
        let close = |a: DVec3, b: DVec3| (a - b).length() < 1e-12;
        assert!(close(arm_direction(0.0, 0.0), DVec3::X)); // north
        assert!(close(arm_direction(PI / 2.0, 0.0), DVec3::Y)); // east
        assert!(close(arm_direction(PI, 0.0), -DVec3::X)); // south
        assert!(close(arm_direction(1.0, PI / 2.0), -DVec3::Z)); // straight up
    }

    #[test]
    fn starts_from_the_south_west_and_above_at_the_default_distance() {
        let target = DVec3::new(100.0, 50.0, -10.0);
        let camera = Camera::new(target);
        let eye = camera.eye();
        // NED: south = less north, west = less east, above = less down.
        assert!(eye.x < target.x && eye.y < target.y && eye.z < target.z);
        assert!(((eye - target).length() - DEFAULT_DISTANCE).abs() < 1e-9);
        assert_eq!(camera.mode, CameraMode::Orbit);
    }

    #[test]
    fn angles_wrap_into_one_turn() {
        assert_eq!(wrap_angle(0.0), 0.0);
        assert!((wrap_angle(3.0 * PI / 2.0) - (-PI / 2.0)).abs() < ROUNDING);
        assert!((wrap_angle(-5.0 * PI / 2.0) - (-PI / 2.0)).abs() < ROUNDING);
        assert_eq!(wrap_angle(PI), -PI); // one way to write it
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
    fn dragging_turns_the_arm_by_a_fixed_angle_per_pixel() {
        let mut camera = Camera::new(DVec3::ZERO);
        camera.orbit(DVec2::new(100.0, -20.0));
        // 100 px right: +0.5 rad of yaw; 20 px up: 0.1 rad lower.
        assert_angle(camera.yaw, START_YAW_DEGREES + 0.5_f64.to_degrees());
        assert_angle(camera.pitch, START_PITCH_DEGREES - 0.1_f64.to_degrees());
        // The arm turns, its length stays.
        assert!((camera.eye().length() - DEFAULT_DISTANCE).abs() < 1e-9);
    }

    #[test]
    fn yaw_goes_round_and_round() {
        let mut camera = Camera::new(DVec3::ZERO);
        // Four turns and a quarter to the right: a quarter turn, written
        // within one turn.
        let pixels = 4.25 * TAU / ORBIT_RADIANS_PER_PIXEL;
        camera.orbit(DVec2::new(pixels, 0.0));
        assert!((-PI..PI).contains(&camera.yaw));
        assert_angle(camera.yaw, START_YAW_DEGREES + 90.0);
    }

    #[test]
    fn pitch_stays_within_limits() {
        let mut camera = Camera::new(DVec3::ZERO);
        camera.orbit(DVec2::new(0.0, 1e6));
        assert_eq!(camera.pitch, MAX_PITCH_DEGREES.to_radians());
        camera.orbit(DVec2::new(0.0, -1e6));
        assert_eq!(camera.pitch, MIN_PITCH_DEGREES.to_radians());
        assert!(camera.eye().z < 0.0, "above the target");
    }

    #[test]
    fn the_gap_to_the_target_halves_every_half_life() {
        let (camera, target) = start();
        let camera = run(camera, target, DVec3::ZERO, HALF_LIFE, HALF_LIFE / 10.0);
        assert!((camera.target.x - 5.0).abs() < 1e-9, "{}", camera.target.x);
        let camera = run(camera, target, DVec3::ZERO, HALF_LIFE, HALF_LIFE / 10.0);
        assert!((camera.target.x - 7.5).abs() < 1e-9, "{}", camera.target.x);
    }

    #[test]
    fn smoothing_does_not_depend_on_the_frame_rate() {
        // Same 1 s of real time at 30, 60 and 144 FPS: same camera target.
        // (A naive `pos += (target - pos) * 0.1` per frame would move
        // almost 5x further at 144 FPS than at 30.)
        let (camera, target) = start();
        let at = |fps: f64| run(camera, target, DVec3::ZERO, 1.0, 1.0 / fps).target;
        assert!((at(30.0) - at(60.0)).length() < 1e-9);
        assert!((at(60.0) - at(144.0)).length() < 1e-9);
    }

    #[test]
    fn camera_ends_up_on_the_target() {
        let (camera, target) = start();
        let camera = run(camera, target, DVec3::ZERO, 5.0, 1.0 / 60.0);
        assert!((camera.target - target).length() < 1e-6);
    }

    #[test]
    fn the_orbit_camera_keeps_its_direction_whatever_the_motion() {
        let camera = Camera::new(DVec3::ZERO);
        let camera = run(
            camera,
            DVec3::ZERO,
            moving_towards(90.0, 30.0),
            5.0,
            1.0 / 60.0,
        );
        assert_angle(camera.yaw, START_YAW_DEGREES);
    }

    #[test]
    fn the_chase_camera_swings_round_behind_the_motion() {
        // Moving east: after a while, the camera looks from the west.
        let camera = run(
            chasing(0.0),
            DVec3::ZERO,
            moving_towards(90.0, 30.0),
            10.0,
            1.0 / 60.0,
        );
        assert_angle(camera.yaw, -90.0);
        let eye = camera.eye();
        assert!(
            eye.y < 0.0 && eye.x.abs() < 0.01,
            "west of the target: {eye}"
        );
    }

    #[test]
    fn the_chase_camera_turns_the_short_way_round() {
        // From 170° to 190° (= -170°): through 180°, never back through 0°.
        let mut camera = chasing(170.0);
        let heading = moving_towards(10.0, 30.0); // behind it: 190°
        for _ in 0..600 {
            camera.update(DVec3::ZERO, heading, 1.0 / 60.0);
            assert!(
                camera.yaw.abs() >= 170.0_f64.to_radians() - 1e-9,
                "{}",
                camera.yaw
            );
        }
        assert_angle(camera.yaw, 190.0);
    }

    #[test]
    fn the_chase_camera_keeps_its_heading_when_motion_has_no_direction() {
        // At rest, or almost: the direction of motion is meaningless.
        let slow = run(
            chasing(30.0),
            DVec3::ZERO,
            moving_towards(90.0, 0.1),
            5.0,
            1.0 / 60.0,
        );
        assert_angle(slow.yaw, 30.0);
        // Falling straight down: no horizontal direction either.
        let falling = DVec3::new(0.0, 0.0, 20.0);
        let falling = run(chasing(30.0), DVec3::ZERO, falling, 5.0, 1.0 / 60.0);
        assert_angle(falling.yaw, 30.0);
    }

    #[test]
    fn the_chase_swing_does_not_depend_on_the_frame_rate() {
        let at = |fps: f64| {
            run(
                chasing(170.0),
                DVec3::ZERO,
                moving_towards(10.0, 30.0),
                0.5,
                1.0 / fps,
            )
            .yaw
        };
        assert!((at(30.0) - at(60.0)).abs() < ROUNDING);
        assert!((at(60.0) - at(144.0)).abs() < ROUNDING);
    }

    #[test]
    fn fixed_camera_stays_put() {
        let (mut camera, target) = start();
        camera.mode = CameraMode::Fixed;
        let camera = run(camera, target, moving_towards(0.0, 30.0), 1.0, 1.0 / 60.0);
        assert_eq!(camera.target, DVec3::ZERO);
        assert_angle(camera.yaw, START_YAW_DEGREES);
    }

    #[test]
    fn modes_cycle_orbit_chase_fixed() {
        let mut camera = Camera::new(DVec3::ZERO);
        let mut names = Vec::new();
        for _ in 0..4 {
            names.push(camera.mode.name());
            camera.next_mode();
        }
        assert_eq!(names, ["orbit", "chase", "fixed", "orbit"]);
    }
}
