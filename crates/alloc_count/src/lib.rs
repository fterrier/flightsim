//! Test-only tool: counts heap allocations, to *check* (not just claim)
//! that a steady-state frame allocates no memory. Used by the engine's and
//! the game's tests (`[dev-dependencies]` only: never in the game itself).
//!
//! Rust lets a program replace its memory allocator (`#[global_allocator]`).
//! A test binary installs [`CountingAllocator`], which forwards to the
//! normal system allocator but counts calls made on the current thread
//! while counting is switched on ([`allocations_in`]). Thread-local, so
//! tests running in parallel on other threads don't disturb the count.
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: alloc_count::CountingAllocator = alloc_count::CountingAllocator;
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// The system allocator, counting. Install it in a test binary with
/// `#[global_allocator]`.
pub struct CountingAllocator;

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

/// Runs `f` and returns how many heap allocations it made on this thread.
/// Only meaningful where `CountingAllocator` is the global allocator.
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

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn the_counter_sees_allocations() {
        // A new Vec allocates once...
        let vec = allocations_in(|| {
            std::hint::black_box(vec![1, 2, 3]);
        });
        assert_eq!(vec, 1);
        // ...arithmetic doesn't.
        let sum = allocations_in(|| {
            std::hint::black_box(2 + 2);
        });
        assert_eq!(sum, 0);
    }
}
