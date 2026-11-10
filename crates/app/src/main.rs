//! The platform shell: opens the window, reads the keyboard and the clock,
//! and runs the game loop.
//!
//! No logic lives here, because this file can't be tested in CI (no
//! display). The loop's logic is in tested modules: `simulation` (fixed
//! steps and interpolation), `camera` (where to look), `render` (how to
//! draw), `hud` (debug text), `view` (world-space framing), and the
//! `physics` crate itself.

#[cfg(test)]
mod alloc_count;
mod camera;
mod hud;
mod kiss3d_backend;
mod render;
mod simulation;
mod timestep;
mod view;

use std::thread;
use std::time::{Duration, Instant};

use kiss3d::event::{Action, Key, WindowEvent};

use camera::FollowCamera;
use hud::{FrameStats, Hud};
use kiss3d_backend::Kiss3dBackend;
use physics::{Body, Circle, DVec2, World};
use render::{Color, Renderer, Shape, Visual};
use simulation::Simulation;

/// Physics runs at this fixed rate, whatever the display's frame rate.
const PHYSICS_HZ: f64 = 120.0;

/// Hold L: every frame takes 50 ms longer, like a slow machine (~15 FPS).
/// The ball still lands at the same moment: that's the fixed timestep at
/// work.
const SLOW_FRAME: Duration = Duration::from_millis(50);

/// Hold K: frames take longer than `timestep::MAX_FRAME_TIME`, so the
/// extra time is dropped and sim time falls behind real time.
const STALLED_FRAME: Duration = Duration::from_millis(400);

/// Colours are linear light (see `render::Color`), hence the tiny values
/// for a dark background.
const BACKGROUND: Color = Color::rgb(0.006, 0.008, 0.015);
const BALL_COLOR: Color = Color::rgb(1.0, 0.25, 0.0);

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

/// What the ball looks like. Its size comes from its collider, so what
/// you see is what collides.
fn ball_visual(world: &World) -> Visual {
    Visual {
        shape: Shape::Circle {
            radius: world.ball_shape.radius as f32,
        },
        color: BALL_COLOR,
    }
}

// `#[kiss3d::main]` generates the real `fn main()`. On desktop it simply
// runs this async function to completion (`pollster::block_on`), and
// `present().await` returns as soon as one frame is drawn: the loop below
// really is ours, running on the main thread. The `async` only exists so
// the same code can run in a browser, where the browser owns the loop.
#[kiss3d::main]
async fn main() {
    let mut backend = Kiss3dBackend::new("flightsim", 1280, 720, BACKGROUND).await;
    let mut sim = new_simulation();
    let renderer = Renderer::new(&mut backend, &ball_visual(sim.world()));

    // Where to look: glides after the ball (see `camera`).
    // TODO(step-05): 3D chase and orbit cameras.
    let mut follow = FollowCamera::new(sim.world().ball.position);
    follow.snap_to(
        sim.world().ball.position,
        renderer.visible_half_height(&backend),
    );

    let mut stats = FrameStats::default();
    let mut hud = Hud::default();
    let mut last_frame: Option<Instant> = None;

    // The game loop: one iteration per displayed frame. The body prepares
    // a frame; the `while` condition then shows it on screen.
    while backend.present().await {
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
        for event in backend.window().events().iter() {
            match event.value {
                WindowEvent::Key(Key::Escape, Action::Press, _) => return,
                WindowEvent::Key(Key::R, Action::Press, _) => sim = new_simulation(),
                WindowEvent::Key(Key::C, Action::Press, _) => follow.toggle_mode(),
                _ => {}
            }
        }
        // TODO(step-06): read throttle and pitch keys into controls and pass
        // them to `sim.advance`.
        // Debug keys act while *held*, so they read the key state instead.
        if backend.window().get_key(Key::L) == Action::Press {
            thread::sleep(SLOW_FRAME);
        }
        if backend.window().get_key(Key::K) == Action::Press {
            thread::sleep(STALLED_FRAME);
        }

        // 3. Physics: as many fixed steps as the elapsed real time calls
        // for. At 60 FPS that's 2 steps per frame; at 144 FPS 0 or 1.
        let steps = sim.advance(frame_time);

        // 4. What to draw: between the last two physics states.
        let drawn = sim.drawn();

        // 5. The camera glides towards what's drawn (in real frame time).
        follow.update(
            drawn.ball,
            frame_time,
            renderer.visible_half_height(&backend),
        );

        // 6. Prepare the frame; the next `present()` shows it.
        hud.update(&sim, steps, &stats, follow.mode);
        renderer.draw_frame(&mut backend, &drawn, follow.position, &hud);
    }
}
