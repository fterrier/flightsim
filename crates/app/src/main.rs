//! The window shell: opens the window, reads the keyboard and the clock,
//! runs the game loop and draws.
//!
//! No logic lives here, because this file can't be tested in CI (no
//! display). The loop's logic is in tested modules: `simulation` (fixed
//! steps and interpolation), `camera` (where to look), `view` (world-space
//! framing), `hud` (screen-space text), and the `physics` crate itself.

#[cfg(test)]
mod alloc_count;
mod camera;
mod hud;
mod simulation;
mod timestep;
mod view;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Everything graphics-related comes from kiss3d, imported explicitly so you
// can see which module each item lives in. `Vec2` is kiss3d's f32 vector
// (glam, via kiss3d's `glamx` wrapper); physics uses its own f64 `DVec2`.
use camera::FollowCamera;
use hud::{FrameStats, Hud};
use kiss3d::camera::PanZoomCamera2d;
use kiss3d::color::{Color, GRAY, WHITE};
use kiss3d::event::{Action, Key, WindowEvent};
use kiss3d::glamx::Vec2;
use kiss3d::scene::SceneNode2d;
use kiss3d::text::Font;
use kiss3d::window::Window;
use physics::{Body, Circle, DVec2, World};
use simulation::{RenderState, Simulation};

/// Physics runs at this fixed rate, whatever the display's frame rate.
const PHYSICS_HZ: f64 = 120.0;

/// Hold L: every frame takes 50 ms longer, like a slow machine (~15 FPS).
/// The ball still lands at the same moment: that's the fixed timestep at
/// work.
const SLOW_FRAME: Duration = Duration::from_millis(50);

/// Hold K: frames take longer than `timestep::MAX_FRAME_TIME`, so the
/// extra time is dropped and sim time falls behind real time.
const STALLED_FRAME: Duration = Duration::from_millis(400);

// Colours are in *linear* light: kiss3d converts them to sRGB (what screens
// expect) when drawing, which brightens dark values a lot. Linear 0.01 shows
// as about 10% grey, not 1%. So dark colours need small numbers here.
const BACKGROUND: Color = Color::new(0.006, 0.008, 0.015, 1.0);
const BALL: Color = Color::new(1.0, 0.25, 0.0, 1.0);

/// The scenario: a ball of this radius (metres), resting on the ground...
const BALL_RADIUS: f64 = 0.4;
/// ...thrown up and forward (m/s): it climbs ~20 m and lands ~33 m away.
const THROW_VELOCITY: DVec2 = DVec2::new(8.0, 20.0);

/// A fresh run of the scenario. Restarting = replacing the `Simulation`,
/// so its clock and interpolation state restart too.
fn new_simulation() -> Simulation {
    let ball = Body {
        position: DVec2::new(0.0, BALL_RADIUS),
        velocity: THROW_VELOCITY,
    };
    let shape = Circle {
        radius: BALL_RADIUS,
    };
    Simulation::new(World::new(ball, shape), 1.0 / PHYSICS_HZ)
}

// `#[kiss3d::main]` generates the real `fn main()`. On desktop it simply
// runs this async function to completion (`pollster::block_on`), and
// `render_2d(...).await` returns as soon as one frame is drawn: the loop
// below really is ours, running on the main thread. The `async` only exists
// so the same code can run in a browser, where the browser owns the loop.
#[kiss3d::main]
async fn main() {
    // Opens the OS window and sets up the GPU (wgpu: Metal on macOS,
    // Vulkan/DX12 elsewhere).
    let mut window = Window::new_with_size("flightsim", 1280, 720).await;
    window.set_background_color(BACKGROUND);
    let font = Font::default();
    let scale = window.scale_factor() as f32;

    let mut sim = new_simulation();

    // Two cameras with separate jobs:
    // - `follow` (ours, tested) decides *where to look*: it glides after
    //   the ball.
    // - `camera` (kiss3d's) does the *projection* from world space (metres,
    //   y up) to the screen. Its zoom is in physical pixels per metre; the
    //   mouse wheel changes it. Mouse-drag panning is off: `follow` steers.
    // TODO(step-05): 3D chase and orbit cameras replace these 2D ones.
    let mut camera = PanZoomCamera2d::new(Vec2::ZERO, view::initial_zoom(scale));
    camera.rebind_drag_button(None);
    let mut follow = FollowCamera::new(sim.world().ball.position);
    let half_height = view::visible_half_height(window.height() as f32, camera.zoom());
    follow.snap_to(sim.world().ball.position, half_height);

    // The *scene*: objects kiss3d keeps between frames (retained mode),
    // positioned in world space. The circle is a "render proxy": a visual
    // stand-in for the physics ball, kept in sync by `sync_scene`. Its size
    // comes from the physics collider, so what you see is what collides.
    let mut scene = SceneNode2d::empty();
    let radius = sim.world().ball_shape.radius as f32;
    let mut ball = scene.add_circle(radius).set_color(BALL);

    let mut stats = FrameStats::default();
    let mut hud = Hud::default();
    let mut last_frame: Option<Instant> = None;

    // The game loop: one iteration per displayed frame. The body prepares
    // a frame; the `while` condition then draws it on screen.
    while window.render_2d(&mut scene, &mut camera).await {
        // 1. Clock: how much real time did the last frame take? The first
        // frame also contains window and GPU setup (~150 ms measured),
        // which must not be simulated, so the clock starts at 0 once the
        // first frame is on screen.
        let now = Instant::now();
        let frame_time = last_frame.map_or(0.0, |last| (now - last).as_secs_f64());
        last_frame = Some(now);
        stats.record(frame_time);

        // 2. Input. Key *presses* come as events, so holding a key acts
        // once, not every frame.
        for event in window.events().iter() {
            match event.value {
                WindowEvent::Key(Key::Escape, Action::Press, _) => return,
                WindowEvent::Key(Key::R, Action::Press, _) => sim = new_simulation(),
                WindowEvent::Key(Key::C, Action::Press, _) => follow.toggle_mode(),
                _ => {}
            }
        }
        // The mouse wheel zooms inside kiss3d's camera; we keep it in range.
        let scale = window.scale_factor() as f32;
        camera.set_zoom(view::clamp_zoom(camera.zoom(), scale));
        // TODO(step-06): read throttle and pitch keys into controls and pass
        // them to `sim.advance`.
        // Debug keys act while *held*, so they read the key state instead.
        if window.get_key(Key::L) == Action::Press {
            thread::sleep(SLOW_FRAME);
        }
        if window.get_key(Key::K) == Action::Press {
            thread::sleep(STALLED_FRAME);
        }

        // 3. Physics: as many fixed steps as the elapsed real time calls
        // for. At 60 FPS that's 2 steps per frame; at 144 FPS 0 or 1.
        let steps = sim.advance(frame_time);

        // 4. What to draw: between the last two physics states.
        let drawn = sim.drawn();

        // 5. The camera glides towards what's drawn (in real frame time).
        let half_height = view::visible_half_height(window.height() as f32, camera.zoom());
        follow.update(drawn.ball, frame_time, half_height);

        // 6. Physics -> scene (see `sync_scene` below).
        sync_scene(&drawn, &follow, &mut ball, &mut camera);

        // 7. Debug drawing and HUD (immediate mode: re-issued every frame).
        hud.update(&sim, steps, &stats, follow.mode);
        draw_ruler(&mut window, &camera);
        draw_hud(&mut window, &font, &hud);
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  PHYSICS -> SCENE
//
//  The only place where simulation state flows into graphics. Physics never
//  knows the scene exists; once per frame we copy what it computed (here
//  already interpolated: `drawn`) into the render proxies, and where
//  to look into kiss3d's camera. Everything after this only *reads*.
//
//  In 3D it stays the same: body position + orientation -> 3D scene node.
// ═══════════════════════════════════════════════════════════════════════
fn sync_scene(
    drawn: &RenderState,
    follow: &FollowCamera,
    ball: &mut SceneNode2d,
    camera: &mut PanZoomCamera2d,
) {
    // f64 (physics precision) -> f32 (what GPUs use).
    // TODO(later): f32 only has ~7 significant digits: at 100 km from the
    // origin, positions snap to ~1 cm steps and the picture jitters. The
    // usual fix is a "floating origin": subtract the camera position in f64
    // *before* converting, so the GPU only sees small numbers.
    // TODO(step-04): physics moves to 3D NED axes (z down); convert to
    // kiss3d's y-up axes here, and only here.
    ball.set_position(Vec2::new(drawn.ball.x as f32, drawn.ball.y as f32));

    let at = follow.position;
    camera.set_at(Vec2::new(at.x as f32, at.y as f32));
}

/// Ground line and distance ruler, in world space (metres).
fn draw_ruler(window: &mut Window, camera: &PanZoomCamera2d) {
    let (from, to) =
        view::visible_x_range(camera.at().x as f64, window.width() as f32, camera.zoom());
    let line_width = 2.0; // in pixels, whatever the zoom
    let point = |x: f64, y: f64| Vec2::new(x as f32, y as f32);

    window.draw_line_2d(point(from, 0.0), point(to, 0.0), GRAY, line_width);
    for tick in view::ruler_ticks(from, to) {
        let (top, bottom) = (point(tick.x, 0.0), point(tick.x, -tick.length));
        window.draw_line_2d(top, bottom, GRAY, line_width);
    }
}

/// HUD text, in screen space (physical pixels, top-left origin).
fn draw_hud(window: &mut Window, font: &Arc<Font>, hud: &Hud) {
    let scale = window.scale_factor() as f32;
    for (i, line) in hud.lines().enumerate() {
        let (x, y) = hud::line_position(i, scale);
        window.draw_text(line, Vec2::new(x, y), hud::font_size(scale), font, WHITE);
    }
}
