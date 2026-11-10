//! The platform shell: loads the scenario, opens the window, and runs the
//! game loop: clock, input, simulation step, render.
//!
//! It doesn't know what's in the world: that's the scenario's business.
//! No logic lives here either, because this file can't be tested in CI (no
//! display). The loop's logic is in tested modules: `simulation` (fixed
//! steps and interpolation), `render` (camera, proxies, HUD), and the
//! `physics` crate itself.

#[cfg(test)]
mod alloc_count;
mod camera;
mod hud;
mod kiss3d_backend;
mod render;
mod scenario;
mod simulation;
mod timestep;
mod view;

use std::thread;
use std::time::{Duration, Instant};

use kiss3d::event::{Action, Key, WindowEvent};

use kiss3d_backend::Kiss3dBackend;
use render::Renderer;
use simulation::Simulation;

/// Physics runs at this fixed rate, whatever the display's frame rate.
const PHYSICS_DT: f64 = 1.0 / 120.0;

/// Hold L: every frame takes 50 ms longer, like a slow machine (~15 FPS).
/// The simulation still keeps real-time pace: that's the fixed timestep at
/// work.
const SLOW_FRAME: Duration = Duration::from_millis(50);

/// Hold K: frames take longer than `timestep::MAX_FRAME_TIME`, so the
/// extra time is dropped and sim time falls behind real time.
const STALLED_FRAME: Duration = Duration::from_millis(400);

// `#[kiss3d::main]` generates the real `fn main()`. On desktop it simply
// runs this async function to completion (`pollster::block_on`), and
// `present().await` returns as soon as one frame is drawn: the loop below
// really is ours, running on the main thread. The `async` only exists so
// the same code can run in a browser, where the browser owns the loop.
#[kiss3d::main]
async fn main() {
    let scenario = scenario::ball_throws();
    let mut backend = Kiss3dBackend::new("flightsim", 1280, 720, scenario.background).await;
    let mut sim = Simulation::new(scenario.world(), PHYSICS_DT);
    let mut renderer = Renderer::new(&mut backend, &scenario);
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

        // 2. Input. Key *presses* come as events, so holding a key acts
        // once, not every frame.
        // TODO(step-06): an input map (keys -> actions and flight controls),
        // tested, instead of matching keys here.
        for event in backend.window().events().iter() {
            match event.value {
                WindowEvent::Key(Key::Escape, Action::Press, _) => return,
                // Restart = reload the scenario into a fresh simulation.
                WindowEvent::Key(Key::R, Action::Press, _) => {
                    sim = Simulation::new(scenario.world(), PHYSICS_DT);
                }
                WindowEvent::Key(Key::C, Action::Press, _) => renderer.toggle_camera(),
                _ => {}
            }
        }
        // Debug keys act while *held*, so they read the key state instead.
        if backend.window().get_key(Key::L) == Action::Press {
            thread::sleep(SLOW_FRAME);
        }
        if backend.window().get_key(Key::K) == Action::Press {
            thread::sleep(STALLED_FRAME);
        }

        // 3. Simulate: as many fixed steps as the elapsed real time calls
        // for. At 60 FPS that's 2 steps per frame; at 144 FPS 0 or 1.
        let steps = sim.advance(frame_time);

        // 4. Render: prepare the frame; the next `present()` shows it.
        renderer.draw_frame(&mut backend, &sim, steps, frame_time);
    }
}
