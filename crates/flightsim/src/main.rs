//! flightsim, the game: what it is (scenarios, later aircraft), how it's
//! played (input bindings) and what it shows (HUD). Everything else (the
//! game loop, physics, camera, rendering) is the engine's.

mod game;
mod scenarios;

fn main() {
    engine_kiss3d::run("flightsim", game::FlightSim);
}
