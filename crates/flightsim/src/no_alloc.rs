//! Checks that the game's per-frame code (input handling, the camera's
//! view, the HUD lines) allocates no memory, with the counting allocator
//! of the `alloc_count` crate (test builds only). The engine checks its
//! own frame the same way.

mod tests {
    use alloc_count::{CountingAllocator, allocations_in};
    use engine::{Body, DVec2, DVec3, Forces, Game, InputEvent, Key, MouseButton, WorldView};

    use crate::game::FlightSim;
    use crate::scenarios::BALL;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn a_steady_state_game_frame_allocates_nothing() {
        let mut game = FlightSim::new();
        // What the engine owns and lends the game, set up beforehand: the
        // bodies, where they're drawn, the forces buffer, the HUD buffer.
        let bodies: Vec<Body> = game.scene().objects.iter().map(|o| o.body).collect();
        let mut drawn: Vec<DVec3> = bodies.iter().map(|b| b.motion.position).collect();
        let mut forces = vec![DVec3::ZERO; bodies.len()];
        let mut hud = String::with_capacity(1024);

        // 10 minutes at 60 FPS, two physics steps a frame: the ball moves,
        // movement keys go down and up, the mouse drags and scrolls all
        // along, the camera goes through every mode and jumps.
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
                for _ in 0..2 {
                    game.fixed_update(&bodies, &mut Forces::new(&mut forces));
                }
                game.input(InputEvent::MouseDragged {
                    button: MouseButton::Left,
                    delta: DVec2::new(1.0, 0.5),
                });
                game.input(InputEvent::Scroll(if frame % 2 == 0 {
                    10.0
                } else {
                    -10.0
                }));
                let time = f64::from(frame) / 60.0;
                drawn[BALL] = DVec3::new(8.0 * time, 0.0, -0.4);
                let world = WorldView {
                    time,
                    bodies: &bodies,
                    drawn: &drawn,
                };
                if frame % 600 == 300 {
                    game.scene_loaded(world); // as after a restart: a jump
                }
                game.view(world, 1.0 / 60.0);
                hud.clear();
                game.write_hud(world, &mut hud).unwrap();
            }
        });
        assert_eq!(count, 0);
    }
}
