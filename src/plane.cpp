#include "plane.h"

namespace fs {

void Plane::updateControls(const fs::InputState &state, double intervalNs) {
  controls.throttle += 0.001 * state.key(fs::InputState::KEY_UP);
  controls.throttle += -0.001 * state.key(fs::InputState::KEY_DOWN);
}

void Plane::updatePower() { _power = controls.throttle; }

void Plane::updateVelocity(double intervalNs) {

  Vector3 gravityVelocity = getGravityVelocityUpdate(velocity, intervalNs);

  updatePower();

  Vector3 thrust = fs::Vector3{intervalNs * 1e-9 * _power, 0, 0};

  Vector3 lift = fs::Vector3{0, -gravityVelocity.y, 0};
  Vector3 drag = fs::Vector3{0, 0, 0};

  velocity = gravityVelocity + lift + thrust + drag;
}

void updatePlaneSimulation(const fs::InputState &state, fs::Plane &plane,
                           double intervalNs) {

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
