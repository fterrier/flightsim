//! Checks that a steady-state frame allocates no memory, with the counting
//! allocator of the `alloc_count` crate (test builds only).

mod tests {
    use crate::engine::Engine;
    use crate::engine::testing::TestGame;
    use crate::input::{MouseButton, RawInput};
    use crate::render::Screen;
    use crate::render::fakes::NullBackend;
    use alloc_count::{CountingAllocator, allocations_in};
    use physics::DVec2;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn a_steady_state_frame_allocates_nothing() {
        // The whole engine runs for real (simulation, input translation,
        // renderer, HUD, the test game's view and HUD lines) against a
        // backend that draws nothing.
        let mut backend = NullBackend(Screen {
            width: 2560.0,
            height: 1440.0,
            scale: 2.0,
        });
        let game = TestGame::new(vec![(-3.0, 6.0), (0.0, 8.0), (3.0, 10.0)], 2);
        let mut engine = Engine::new(&mut backend, game);

        // 10 simulated minutes of frames: numbers on the HUD grow longer,
        // the FPS readout appears... still no allocation. The mouse drags
        // all along.
        let count = allocations_in(|| {
            engine.input(RawInput::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            });
            for frame in 0..36_000 {
                let x = f64::from(frame % 1000);
                engine.input(RawInput::CursorMoved(DVec2::new(x, 500.0)));
                engine.frame(&mut backend, 1.0 / 60.0);
            }
        });
        assert_eq!(count, 0);
    }
}
