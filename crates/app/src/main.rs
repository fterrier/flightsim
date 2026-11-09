//! The window shell: opens the window, reads the keyboard and the clock,
//! runs the game loop and draws.
//!
//! No logic lives here, because this file can't be tested in CI (no
//! display). Timing is in `timestep`, what-to-show-where in `view`, and the
//! simulation itself in the `physics` crate.

mod timestep;
mod view;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Everything graphics-related comes from kiss3d, imported explicitly so you
// can see which module each item lives in. `Vec2` is kiss3d's f32 vector
// (glam, via kiss3d's `glamx` wrapper); physics uses its own f64 `DVec2`.
use kiss3d::camera::PanZoomCamera2d;
use kiss3d::color::{Color, GRAY, WHITE};
use kiss3d::event::{Action, Key};
use kiss3d::glamx::Vec2;
use kiss3d::scene::SceneNode2d;
use kiss3d::text::Font;
use kiss3d::window::Window;
use physics::{Body, DVec2, World};
use timestep::FixedTimestep;

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

/// The scenario: a ball of this radius (metres)...
const BALL_RADIUS: f64 = 0.4;
/// ...starting this high (metres)...
const START_HEIGHT: f64 = 20.0;
/// ...thrown sideways at this speed (m/s), so it flies a parabola.
const THROW_SPEED: f64 = 5.0;

/// A fresh run: the ball at its starting point, and a new clock so that
/// sim time and real time both restart from zero.
fn new_run() -> (World, FixedTimestep) {
    let ball = Body {
        position: DVec2::new(0.0, START_HEIGHT),
        velocity: DVec2::new(THROW_SPEED, 0.0),
    };
    (
        World::new(ball, BALL_RADIUS),
        FixedTimestep::new(1.0 / PHYSICS_HZ),
    )
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

    // The camera maps world space (metres, y up) to the screen. Its zoom is
    // in physical pixels per metre; the mouse wheel changes it. Mouse-drag
    // panning is turned off because the camera follows the ball.
    let mut camera = PanZoomCamera2d::new(Vec2::ZERO, view::initial_zoom(scale));
    camera.rebind_drag_button(None);

    let (mut world, mut clock) = new_run();

    // The *scene*: objects kiss3d keeps between frames (retained mode),
    // positioned in world space. The circle is a "render proxy": a visual
    // stand-in for the physics ball, kept in sync by `sync_scene`. Its size
    // comes from the physics collision radius, so what you see is what
    // collides.
    let mut scene = SceneNode2d::empty();
    let mut ball = scene.add_circle(world.ball_radius as f32).set_color(BALL);

    let mut last_frame = Instant::now();

    // The game loop: one iteration per displayed frame. The body prepares
    // a frame; the `while` condition then draws it on screen.
    while window.render_2d(&mut scene, &mut camera).await {
        // 1. Clock: how much real time did the last frame take?
        let now = Instant::now();
        let frame_time = (now - last_frame).as_secs_f64();
        last_frame = now;

        // 2. Input
        if window.get_key(Key::Escape) == Action::Press {
            break;
        }
        if window.get_key(Key::R) == Action::Press {
            (world, clock) = new_run();
        }
        // TODO(step-03): read throttle and pitch keys into controls and pass
        // them to `world.step`.
        if window.get_key(Key::L) == Action::Press {
            thread::sleep(SLOW_FRAME);
        }
        if window.get_key(Key::K) == Action::Press {
            thread::sleep(STALLED_FRAME);
        }

        // 3. Update physics: as many fixed steps as the elapsed real time
        // calls for. At 60 FPS that's 2 steps per frame; at 144 FPS
        // sometimes 0 or 1.
        let steps = clock.advance(frame_time);
        for _ in 0..steps {
            // TODO(step-07): keep a copy of the previous state here so the
            // renderer can interpolate between the last two physics states.
            world.step(clock.dt());
        }

        // 4. Physics -> scene (see `sync_scene` below).
        sync_scene(&world, &mut ball, &mut camera);

        // 5. Debug drawing and HUD (immediate mode: re-issued every frame).
        draw_ruler(&mut window, &camera);
        draw_hud(&mut window, &font, &world, &clock, steps, frame_time);
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  PHYSICS -> SCENE
//
//  The only place where simulation state flows into graphics. Physics never
//  knows the scene exists; once per frame we copy what it computed into the
//  render proxies (and the camera). Everything after this only *reads*.
//
//  In 3D it stays the same: body position + orientation -> 3D scene node.
// ═══════════════════════════════════════════════════════════════════════
fn sync_scene(world: &World, ball: &mut SceneNode2d, camera: &mut PanZoomCamera2d) {
    // f64 (physics precision) -> f32 (what GPUs use).
    // TODO(later): f32 only has ~7 significant digits: at 100 km from the
    // origin, positions snap to ~1 cm steps and the picture jitters. The
    // usual fix is a "floating origin": subtract the camera position in f64
    // *before* converting, so the GPU only sees small numbers.
    let position = world.ball.position;
    ball.set_position(Vec2::new(position.x as f32, position.y as f32));

    let target = view::camera_target(position);
    camera.set_at(Vec2::new(target.x as f32, target.y as f32));
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

/// Text overlay, in screen space (physical pixels, top-left origin).
fn draw_hud(
    window: &mut Window,
    font: &Arc<Font>,
    world: &World,
    clock: &FixedTimestep,
    steps: u32,
    frame_time: f64,
) {
    let scale = window.scale_factor() as f32;
    let lines = view::overlay_lines(world, clock, steps, frame_time);
    for (i, line) in lines.iter().enumerate() {
        let (x, y) = view::hud_line_position(i, scale);
        window.draw_text(
            line,
            Vec2::new(x, y),
            view::hud_font_size(scale),
            font,
            WHITE,
        );
    }
}
