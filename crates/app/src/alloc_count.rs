//! Test-only tool: counts heap allocations, to *check* (not just claim)
//! that a steady-state frame allocates no memory.
//!
//! Rust lets a program replace its memory allocator (`#[global_allocator]`).
//! For the test binary only, we install one that forwards to the normal
//! system allocator, but counts calls made on the current thread while
//! counting is switched on. Thread-local, so tests running in parallel on
//! other threads don't disturb the count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    // `const` initialisation: reading these never allocates itself.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every call is forwarded unchanged to `System`, the default
// allocator, which upholds the `GlobalAlloc` contract; we only count.
// (`realloc` and `alloc_zeroed` have default implementations built on
// `alloc`, so they are counted too.)
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.get() {
            ALLOCATIONS.set(ALLOCATIONS.get() + 1);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `f` and returns how many heap allocations it made on this thread.
pub fn allocations_in(f: impl FnOnce()) -> usize {
    ALLOCATIONS.set(0);
    COUNTING.set(true);
    f();
    COUNTING.set(false);
    ALLOCATIONS.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraMode, FollowCamera};
    use crate::hud::{FrameStats, Hud};
    use crate::render::fakes::NullBackend;
    use crate::render::{Renderer, Screen, Shape, Visual, WHITE};
    use crate::simulation::Simulation;
    use physics::{Body, Circle, DVec2, World};

    #[test]
    fn the_counter_sees_allocations() {
        // Sanity check of the tool itself: a new Vec allocates once.
        let count = allocations_in(|| {
            std::hint::black_box(vec![1, 2, 3]);
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn a_steady_state_frame_allocates_nothing() {
        let ball = Body {
            position: DVec2::new(0.0, 0.4),
            velocity: DVec2::new(8.0, 20.0),
        };
        let mut sim = Simulation::new(World::new(ball, Circle { radius: 0.4 }), 1.0 / 120.0);
        let mut camera = FollowCamera::new(DVec2::ZERO);
        let mut stats = FrameStats::default();
        let mut hud = Hud::default();
        // The renderer runs for real, against a backend that draws nothing.
        let mut backend = NullBackend(Screen {
            width: 2560.0,
            height: 1440.0,
            scale: 2.0,
        });
        let visual = Visual {
            shape: Shape::Circle { radius: 0.4 },
            color: WHITE,
        };
        let renderer = Renderer::new(&mut backend, &visual);

        // Everything the game loop does per frame; only kiss3d itself is
        // replaced (by `NullBackend`).
        let mut frame = || {
            let frame_time = 1.0 / 60.0;
            stats.record(frame_time);
            let steps = sim.advance(frame_time);
            let drawn = sim.drawn();
            let half_height = renderer.visible_half_height(&backend);
            camera.update(drawn.ball, frame_time, half_height);
            hud.update(&sim, steps, &stats, CameraMode::Follow);
            renderer.draw_frame(&mut backend, &drawn, camera.position, &hud);
        };

        // 10 simulated minutes of frames: numbers on the HUD grow longer,
        // the ball lands, the FPS readout appears... still no allocation.
        let count = allocations_in(|| {
            for _ in 0..36_000 {
                frame();
            }
        });
        assert_eq!(count, 0);
    }
}
