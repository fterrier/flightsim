//! Fixed-timestep clock: turns irregular frame times into a whole number of
//! equal physics steps.
//!
//! Frames never last exactly the same time (16.6 ms, then 17.1 ms, then a
//! 40 ms hiccup...). If physics used each frame's duration as its `dt`, the
//! results would depend on the frame rate: a fast machine and a slow one
//! would fly different trajectories, and nothing would be reproducible.
//!
//! Instead, real time is poured into an *accumulator*, and physics consumes
//! it in fixed slices of `dt`. Whatever doesn't fill a whole slice stays in
//! the accumulator for the next frame.
//!
//! Reference: Glenn Fiedler, "Fix Your Timestep!"

/// The longest frame time we accept, in seconds. If a frame takes longer
/// (debugger pause, window drag), the extra time is dropped: the simulation
/// briefly runs slower than real time instead of trying to catch up.
///
/// Without this cap, a slow frame means many physics steps next frame,
/// which makes that frame slow too, which means even more steps... the
/// "spiral of death".
pub(crate) const MAX_FRAME_TIME: f64 = 0.25;

pub(crate) struct FixedTimestep {
    /// Duration of one physics step, in seconds.
    dt: f64,
    /// Real time not yet consumed by physics steps, in seconds.
    accumulator: f64,
}

impl FixedTimestep {
    pub(crate) fn new(dt: f64) -> Self {
        Self {
            dt,
            accumulator: 0.0,
        }
    }

    /// Feeds the real time the last frame took (in seconds) and returns how
    /// many physics steps to run now.
    pub(crate) fn advance(&mut self, frame_time: f64) -> u32 {
        self.accumulator += frame_time.min(MAX_FRAME_TIME);

        let mut steps = 0;
        while self.accumulator >= self.dt {
            self.accumulator -= self.dt;
            steps += 1;
        }
        steps
    }

    /// How far we are from the last physics step towards the next one, as a
    /// fraction of a step: 0.0 = exactly at the latest state, 0.99 = almost
    /// at the next one.
    ///
    /// The renderer uses it to draw *between* the previous and the current
    /// physics state (interpolation). Without it, the drawn position only
    /// changes when a step happens; when frames get 1, then 2, then 1 steps,
    /// motion visibly stutters ("judder").
    pub(crate) fn alpha(&self) -> f64 {
        self.accumulator / self.dt
    }

    pub(crate) fn dt(&self) -> f64 {
        self.dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1/64 s is exactly representable in binary floating point, so these
    // tests can compare step counts exactly without rounding surprises.
    const DT: f64 = 1.0 / 64.0;

    #[test]
    fn no_step_until_a_full_dt_has_accumulated() {
        let mut clock = FixedTimestep::new(DT);
        assert_eq!(clock.advance(DT / 2.0), 0);
        assert_eq!(clock.advance(DT / 2.0), 1);
    }

    #[test]
    fn leftover_time_carries_over_to_the_next_frame() {
        let mut clock = FixedTimestep::new(DT);
        assert_eq!(clock.advance(2.5 * DT), 2); // 0.5 dt left over...
        assert_eq!(clock.advance(0.5 * DT), 1); // ...completes a step here.
    }

    #[test]
    fn long_frame_is_capped_to_avoid_the_spiral_of_death() {
        let mut clock = FixedTimestep::new(DT);
        // A 10 s freeze only produces MAX_FRAME_TIME worth of steps: the
        // simulation falls behind real time instead of catching up.
        assert_eq!(clock.advance(10.0), (MAX_FRAME_TIME / DT) as u32);
    }

    #[test]
    fn simulation_keeps_pace_with_real_time_at_any_frame_rate() {
        for fps in [30.0, 60.0, 144.0, 1000.0] {
            let mut clock = FixedTimestep::new(1.0 / 120.0);
            let frames = fps as u32; // one second of frames
            let steps: u32 = (0..frames).map(|_| clock.advance(1.0 / fps)).sum();
            // 1 s at 120 Hz = 120 steps. Rounding may leave the last step
            // in the accumulator, hence 119 is accepted too.
            assert!((119..=120).contains(&steps), "{fps} fps gave {steps} steps");
        }
    }

    #[test]
    fn exposes_its_timestep() {
        assert_eq!(FixedTimestep::new(DT).dt(), DT);
    }

    #[test]
    fn alpha_is_how_far_we_are_towards_the_next_step() {
        let mut clock = FixedTimestep::new(DT);
        assert_eq!(clock.alpha(), 0.0);
        clock.advance(2.25 * DT); // 2 steps run, a quarter step left over
        assert_eq!(clock.alpha(), 0.25);
        clock.advance(0.5 * DT); // no step: just further along
        assert_eq!(clock.alpha(), 0.75);
    }
}
