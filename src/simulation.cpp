#include "simulation.h"
#include "fs_math.h"

#include <spdlog/fmt/ostr.h>
#include <spdlog/spdlog.h>

using namespace std;

namespace fs {

bool InputState::key(short key) const {
  return keys & key;
}
void InputState::setKey(short key) {
  keys |= key;
}
void InputState::unsetKey(short key) {
  keys ^= key;
}

const Vector3 gravityVector = Vector3{0, -1.0F, 0};
const double gravity = 9.81;

Vector3 getGravityVelocityUpdate(const Vector3 &velocity, double intervalNs) {
  return velocity + gravityVector * gravity * intervalNs * 1e-9;
}

void BasicObject::updatePosition() { position = position + velocity; }

} // namespace fs
