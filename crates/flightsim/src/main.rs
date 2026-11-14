//! flightsim, the game: what it is (scenarios, later aircraft), how it's
//! played (its controls and camera) and what it shows (HUD). Everything
//! else (the game loop, physics, input translation, rendering) is the
//! engine's.

mod camera;
mod game;
#[cfg(test)]
mod no_alloc;
mod scenarios;
mod textures;

fn main() {
    engine_kiss3d::run("flightsim", game::FlightSim::new);
}
