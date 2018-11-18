#include "plane.h"

#include <spdlog/spdlog.h>

// auto console = spdlog::stdout_logger_mt("plane");

const double rho = 1.225; // kg/m3
const Vector3 gravityVector = Vector3{0, -1.0F, 0};
const double gravity = 9.81;

// from input to control
void Plane::updateControls(const InputState &state, double intervalNs) {
  controls.throttle += 0.001 * state.key(InputState::KEY_UP);
  controls.throttle += -0.001 * state.key(InputState::KEY_DOWN);

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

double Plane::calculateLiftCoefficient() {
  return 5;
}

Vector3 Plane::calculateLift(Vector3 velocity) {
  // 1/2 * rho * v^2 * S * Cl - along y axis
  return Vector3{0, 1, 0} * (1/2) * rho * _wingArea
                          * calculateLiftCoefficient()
                          * (velocity.x * velocity.x);
}

void Plane::updateVelocity(double intervalNs) {

  updatePower();

  Vector3 thrust = Vector3{0, 0, _power};
  Vector3 weight = gravityVector * gravity * _weight;
  Vector3 lift = calculateLift(velocity);
  Vector3 drag = Vector3{0, 0, 0};

  velocity = (weight + lift + thrust + drag) * 1e-10 * intervalNs;
}
// up to here


void updatePlaneSimulation(const InputState &state, Plane &plane, double intervalNs) {

  // 1. based on the input state, we set the plane controls to the correct
  // values
  plane.updateControls(state, intervalNs);

  // 2. based on the new controls, we get the plane current lift, drag and
  // thrust
  plane.updateVelocity(intervalNs);

  // 3. we update the position
  plane.updatePosition();
}
