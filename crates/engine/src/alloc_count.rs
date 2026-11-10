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
    use crate::Engine;
    use crate::engine::testing::TestGame;
    use crate::render::Screen;
    use crate::render::fakes::NullBackend;

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
        // The whole engine runs for real (simulation, camera, renderer,
        // HUD, the game's HUD lines) against a backend that draws nothing.
        let mut backend = NullBackend(Screen {
            width: 2560.0,
            height: 1440.0,
            scale: 2.0,
        });
        let game = TestGame {
            balls: vec![(-3.0, 6.0), (0.0, 8.0), (3.0, 10.0)],
            focus: 1,
        };
        let mut engine = Engine::new(&mut backend, game);

        // 10 simulated minutes of frames: numbers on the HUD grow longer,
        // the FPS readout appears... still no allocation.
        let count = allocations_in(|| {
            for _ in 0..36_000 {
                engine.frame(&mut backend, 1.0 / 60.0);
            }
        });
        assert_eq!(count, 0);
    }
}
