//! The window shell: opens the window, reads the keyboard, runs the game
//! loop and draws.
//!
//! No logic lives here, because this file can't be tested in CI (no
//! display). Timing is in `timestep`, what-goes-where in `view`, and the
//! simulation itself in the `physics` crate.

mod timestep;
mod view;

use std::thread;
use std::time::Duration;

// Everything graphics-related comes from macroquad, imported explicitly so
// you can see which module each function lives in.
use macroquad::color::{Color, GRAY, ORANGE, WHITE};
use macroquad::input::{KeyCode, is_key_down, is_key_pressed};
use macroquad::shapes::{draw_circle, draw_line};
use macroquad::text::draw_text;
use macroquad::time::{get_fps, get_frame_time};
use macroquad::window::{Conf, clear_background, next_frame, screen_height, screen_width};
use physics::{Body, DVec2, World};
use timestep::FixedTimestep;
use view::View;

/// Physics runs at this fixed rate, whatever the display's frame rate.
const PHYSICS_HZ: f64 = 120.0;

/// Hold L: every frame takes 50 ms longer, like a slow machine (~15 FPS).
/// The dot keeps the same speed: that's the fixed timestep at work.
const SLOW_FRAME: Duration = Duration::from_millis(50);

/// Hold K: frames take longer than `timestep::MAX_FRAME_TIME`, so the
/// extra time is dropped and sim time falls behind real time.
const STALLED_FRAME: Duration = Duration::from_millis(400);

fn window_conf() -> Conf {
    Conf {
        window_title: "flightsim".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

// This attribute is where the window comes from. It's a macro: at compile
// time it renames this function and generates the real `fn main()`, which
// calls `macroquad::Window::from_config(window_conf(), <this function>)`.
// That:
// 1. opens the OS window and GPU context (through miniquad, macroquad's
//    lower layer), sized and titled by `window_conf()` above;
// 2. stores the drawing state in a global, which is why `draw_line(...)`
//    needs no window handle;
// 3. runs this `async fn` as a coroutine. Each `next_frame().await` hands
//    control back to the OS event loop, which presents the frame, processes
//    events (keys, resize, close) and resumes us on the next frame.
// So the OS event loop really drives us; `async` just lets our side read
// like a plain `loop {}`.
// TODO(later): with our own wgpu renderer we create the window ourselves
// (winit) and own the event loop explicitly.
#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new(Body {
        position: DVec2::new(0.0, 2.0),
        velocity: DVec2::new(10.0, 0.0),
    });
    let mut clock = FixedTimestep::new(1.0 / PHYSICS_HZ);

    // The game loop: one iteration per displayed frame.
    loop {
        // 1. Input
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        // TODO(step-03): read throttle and pitch keys into controls and pass
        // them to `world.step`.
        if is_key_down(KeyCode::L) {
            thread::sleep(SLOW_FRAME);
        }
        if is_key_down(KeyCode::K) {
            thread::sleep(STALLED_FRAME);
        }

        // 2. Update: as many fixed steps as the elapsed real time calls for.
        // At 60 FPS that's 2 steps per frame; at 144 FPS sometimes 0 or 1.
        let steps = clock.advance(get_frame_time() as f64);
        for _ in 0..steps {
            // TODO(step-07): keep a copy of the previous state here so the
            // renderer can interpolate between the last two physics states.
            world.step(clock.dt());
        }

        // 3. Render: reads the world, never changes it.
        draw(&world, &clock, steps);
        next_frame().await;
    }
}

fn draw(world: &World, clock: &FixedTimestep, steps: u32) {
    let view = View::for_screen(screen_width(), screen_height());
    clear_background(Color::from_rgba(20, 24, 32, 255));

    // Ground and distance ruler.
    draw_line(0.0, view.ground_y, screen_width(), view.ground_y, 2.0, GRAY);
    for tick in view.ruler_ticks() {
        draw_line(
            tick.x,
            view.ground_y,
            tick.x,
            view.ground_y + tick.length,
            1.0,
            GRAY,
        );
        if let Some(label) = tick.label {
            draw_text(&label, tick.x + 3.0, view.ground_y + 28.0, 18.0, GRAY);
        }
    }

    let (x, y) = view.to_screen(world.dot.position);
    draw_circle(x, y, 8.0, ORANGE);

    for (i, line) in view::overlay_lines(world, clock, steps, get_fps())
        .iter()
        .enumerate()
    {
        draw_text(line, 12.0, 28.0 + 24.0 * i as f32, 24.0, WHITE);
    }
}
