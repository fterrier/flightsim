#include "plane.h"

#include <spdlog/spdlog.h>

namespace fs {

auto console = spdlog::stdout_logger_mt("plane");

const Vector3 gravityVector = Vector3{0, -1.0F, 0};
const double gravity = 9.81;

// from input to control
void Plane::updateControls(const fs::InputState &state, double intervalNs) {
  controls.throttle += 0.001 * state.key(fs::InputState::KEY_UP);
  controls.throttle += -0.001 * state.key(fs::InputState::KEY_DOWN);

  if (controls.throttle > 100) {
    controls.throttle = 100;
  }
  if (controls.throttle < 0) {
    controls.throttle = 0;
  }
}
// to here

// this is the actual simulation
void Plane::updatePower() {

  // TODO this should lag behind the actual throttle updates
  _power = controls.throttle;
}


Controls* Plane::getControls() {
  return &controls;
}

void Plane::updateVelocity(double intervalNs) {

  updatePower();

  Vector3 thrust = fs::Vector3{_power, 0, 0};
  Vector3 weight = gravityVector * gravity * _weight;
  Vector3 lift = -weight;
  Vector3 drag = fs::Vector3{0, 0, 0};

  velocity = (weight + lift + thrust + drag) * 1e-4 * intervalNs;

  console->info("Velocity: {0:f}, {1:f}, {2:f}", velocity.x, velocity.y, velocity.z);
}
// up to here


void updatePlaneSimulation(const fs::InputState &state, fs::Plane &plane, double intervalNs) {

  // 1. based on the input state, we set the plane controls to the correct
  // values
  plane.updateControls(state, intervalNs);

  // 2. based on the new controls, we get the plane current lift, drag and
  // thrust
  plane.updateVelocity(intervalNs);

  // 3. we update the position
  plane.updatePosition();
}

} // namespace fs
