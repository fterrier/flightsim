//! Flight physics: pure, deterministic simulation code.
//!
//! Rules for this crate (see ROADMAP.md):
//! - No I/O, no clock, no graphics. Time only enters as a `dt` argument.
//! - Same inputs => same outputs, bit for bit. This is what makes tests
//!   and (later) replays possible.
//! - SI units everywhere: metres, seconds, kilograms, radians.

#[cfg(test)]
mod tests {
    #[test]
    fn harness_works() {
        assert_eq!(1 + 1, 2);
    }
}
