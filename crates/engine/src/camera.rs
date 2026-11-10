//! The camera: where it looks, how far it's zoomed, and so what part of the
//! world is visible. Pure logic in world space (metres), tested without a
//! window; the renderer hands the result to the backend for projection.
//!
//! Engines keep all of this in one camera object: Godot's `Camera2D` has
//! position smoothing, drag margins (our dead zone), limits and zoom;
//! Unity's `Camera.orthographicSize` is half the visible height in world
//! units, like `visible_half_extents` here.
//!
//! A good game camera doesn't snap to its target: it *glides* towards it,
//! and doesn't move at all for small motions (a dead zone), so the view
//! feels calm. This uses real frame time, not physics time: the camera is
//! presentation, not simulation. Framing is expressed as fractions of
//! what's visible, not in metres, so it keeps working at any zoom.
//!
//! Sizes on screen are in *logical* pixels (independent of HiDPI); the
//! renderer converts to physical pixels.

use physics::DVec2;

/// Starting zoom, logical pixels per metre.
pub const DEFAULT_ZOOM: f32 = 15.0;

/// Zoom limits, logical pixels per metre: the mouse wheel can't zoom into a
/// blur or out to nothing.
pub const MIN_ZOOM: f32 = 3.0;
pub const MAX_ZOOM: f32 = 60.0;

/// Scroll distance (pixels; a mouse-wheel notch counts as 10) that doubles
/// the zoom. Zoom changes exponentially with scrolling, so each notch
/// scales the view by the same ratio at any zoom level.
const ZOOM_DOUBLING: f64 = 300.0;

/// Time for the camera to close half the gap to where it wants to be, in
/// seconds. Smaller = snappier, larger = floatier.
const HALF_LIFE: f64 = 0.15;

/// The target can move this far above or below the camera centre before
/// the camera follows vertically, as a fraction of the visible half-height
/// (0.2: the target may roam the middle 20% of the screen height). Godot
/// calls these "drag margins".
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
pub struct Camera {
    /// The point the camera looks at (centre of the screen), in metres.
    pub position: DVec2,
    pub mode: CameraMode,
    /// Logical pixels per metre. Private: only `zoom_by` changes it, so it
    /// always stays within the limits.
    zoom: f32,
}

impl Camera {
    pub fn new(position: DVec2) -> Self {
        Self {
            position,
            mode: CameraMode::Follow,
            zoom: DEFAULT_ZOOM,
        }
    }

    /// Logical pixels per metre.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Zooms in (positive `scroll`) or out (negative), `scroll` being the
    /// scroll distance in pixels.
    pub fn zoom_by(&mut self, scroll: f64) {
        let factor = 2.0_f64.powf(scroll / ZOOM_DOUBLING);
        self.zoom = (self.zoom as f64 * factor).clamp(MIN_ZOOM as f64, MAX_ZOOM as f64) as f32;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            CameraMode::Follow => CameraMode::Fixed,
            CameraMode::Fixed => CameraMode::Follow,
        };
    }

    /// Half the visible width and height, in metres, for a `viewport` of
    /// that size in logical pixels.
    pub fn visible_half_extents(&self, viewport: DVec2) -> DVec2 {
        viewport / self.zoom as f64 / 2.0
    }

    /// Where the camera would like to be to frame `target` (metres):
    /// centred on it horizontally; vertically, moved only as far as needed
    /// to keep the target inside the dead zone, and never so low that the
    /// ground rises into the upper part of the screen.
    pub fn desired_position(&self, target: DVec2, viewport: DVec2) -> DVec2 {
        let half_height = self.visible_half_extents(viewport).y;
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
    pub fn snap_to(&mut self, target: DVec2, viewport: DVec2) {
        let half_height = self.visible_half_extents(viewport).y;
        self.position = DVec2::new(target.x, target.y.max(MIN_HEIGHT * half_height));
    }

    /// Moves the camera towards `target` for one frame lasting
    /// `frame_time` seconds, on a `viewport` (logical pixels).
    pub fn update(&mut self, target: DVec2, frame_time: f64, viewport: DVec2) {
        if self.mode == CameraMode::Fixed {
            return;
        }
        let desired = self.desired_position(target, viewport);
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

    /// A 800 x 600 logical-pixel viewport. At the default zoom (15 px/m)
    /// it shows 20 m above and below the camera centre.
    const VIEWPORT: DVec2 = DVec2::new(800.0, 600.0);
    const H: f64 = 20.0;
    /// Lowest camera height at that zoom.
    const LOWEST: f64 = MIN_HEIGHT * H;
    /// Dead zone half-height at that zoom.
    const ZONE: f64 = DEAD_ZONE * H;

    /// Moves `camera` towards a fixed `target` for `seconds`, in frames of
    /// `frame_time`.
    fn run(mut camera: Camera, target: DVec2, seconds: f64, frame_time: f64) -> Camera {
        let frames = (seconds / frame_time).round() as u32;
        for _ in 0..frames {
            camera.update(target, frame_time, VIEWPORT);
        }
        camera
    }

    /// A camera at its lowest height and a target 10 m to the right at the
    /// same height (inside the dead zone, so only x moves).
    fn start() -> (Camera, DVec2) {
        (
            Camera::new(DVec2::new(0.0, LOWEST)),
            DVec2::new(10.0, LOWEST),
        )
    }

    #[test]
    fn starts_at_the_default_zoom() {
        assert_eq!(Camera::new(DVec2::ZERO).zoom(), DEFAULT_ZOOM);
    }

    #[test]
    fn visible_area_follows_from_viewport_and_zoom() {
        // Like Unity's `orthographicSize`: half the visible height, in
        // world units. 600 px at 15 px/m = 40 m tall, 800 px = 53.3 m wide.
        let camera = Camera::new(DVec2::ZERO);
        assert_eq!(
            camera.visible_half_extents(VIEWPORT),
            DVec2::new(800.0 / 15.0 / 2.0, H)
        );
    }

    #[test]
    fn scrolling_up_zooms_in_and_down_zooms_out() {
        let mut camera = Camera::new(DVec2::ZERO);
        camera.zoom_by(ZOOM_DOUBLING);
        assert!((camera.zoom() - 2.0 * DEFAULT_ZOOM).abs() < 1e-4);
        camera.zoom_by(-ZOOM_DOUBLING);
        // Exponential: in then out by the same amount comes back exactly.
        assert!((camera.zoom() - DEFAULT_ZOOM).abs() < 1e-4);
    }

    #[test]
    fn zoom_is_kept_within_limits() {
        let mut camera = Camera::new(DVec2::ZERO);
        camera.zoom_by(1e6);
        assert_eq!(camera.zoom(), MAX_ZOOM);
        camera.zoom_by(-1e6);
        assert_eq!(camera.zoom(), MIN_ZOOM);
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
        let camera = Camera::new(DVec2::new(0.0, 50.0));
        let target = DVec2::new(0.0, 50.0 + ZONE * 0.9);
        assert_eq!(camera.desired_position(target, VIEWPORT).y, 50.0);
    }

    #[test]
    fn camera_moves_just_enough_to_keep_the_target_in_the_dead_zone() {
        let camera = Camera::new(DVec2::new(0.0, 50.0));
        let above = DVec2::new(0.0, 50.0 + ZONE + 3.0);
        assert_eq!(camera.desired_position(above, VIEWPORT).y, 53.0);
        let below = DVec2::new(0.0, 50.0 - ZONE - 3.0);
        assert_eq!(camera.desired_position(below, VIEWPORT).y, 47.0);
    }

    #[test]
    fn ground_stays_in_the_lower_part_of_the_screen_at_any_zoom() {
        // Framing is a fraction of what's visible, not a fixed height in
        // metres: zoomed in, the camera may go lower.
        let on_ground = DVec2::new(0.0, 0.0);
        for scroll in [-ZOOM_DOUBLING, 0.0, ZOOM_DOUBLING] {
            let mut camera = Camera::new(DVec2::ZERO);
            camera.zoom_by(scroll);
            let half_height = camera.visible_half_extents(VIEWPORT).y;
            let y = camera.desired_position(on_ground, VIEWPORT).y;
            // Ground (y = 0) is `y` below the centre: within the visible
            // half-height, and in the lower half of the screen.
            assert_eq!(y, MIN_HEIGHT * half_height);
            assert!(y > 0.0 && y < half_height);
        }
    }

    #[test]
    fn snap_frames_the_target_immediately() {
        let mut camera = Camera::new(DVec2::ZERO);
        camera.snap_to(DVec2::new(50.0, 30.0), VIEWPORT);
        assert_eq!(camera.position, DVec2::new(50.0, 30.0));
        camera.snap_to(DVec2::new(0.0, 0.0), VIEWPORT); // on the ground
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
