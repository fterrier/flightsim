//! Checks that the game, run by the real engine, allocates no memory once
//! running: input handling, physics-step logic, the camera's view, the HUD
//! lines. With the counting allocator of the `alloc_count` crate (test
//! builds only). The engine checks its own frame the same way.

mod tests {
    use alloc_count::{CountingAllocator, allocations_in};
    use engine::{DVec2, Headless, InputEvent, Key, MouseButton};

    use crate::game::FlightSim;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn a_steady_state_game_frame_allocates_nothing() {
        // Loading allocates (the scene, the world, buffers), like any level
        // load, and so does reloading (R): not counted. Playing must not.
        let mut game = Headless::new(FlightSim::new);

        // 10 minutes at 60 FPS: movement keys go down and up, the mouse
        // drags and scrolls all along, the camera goes through every mode.
        let count = allocations_in(|| {
            for frame in 0..36_000 {
                if frame % 600 == 0 {
                    game.input(InputEvent::KeyPressed(Key::C));
                }
                let key = [Key::W, Key::A, Key::S, Key::D][frame as usize / 60 % 4];
                if frame % 60 == 0 {
                    game.input(InputEvent::KeyPressed(key));
                } else if frame % 60 == 30 {
                    game.input(InputEvent::KeyReleased(key));
                }
                game.input(InputEvent::MouseDragged {
                    button: MouseButton::Left,
                    delta: DVec2::new(1.0, 0.5),
                });
                let scroll = if frame % 2 == 0 { 10.0 } else { -10.0 };
                game.input(InputEvent::Scroll(scroll));
                game.frame(1.0 / 60.0);
            }
        });
        assert_eq!(count, 0);
    }
}
