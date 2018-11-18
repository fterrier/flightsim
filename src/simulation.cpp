#include "simulation.h"
#include "fs_math.h"

bool InputState::key(short key) const {
  return keys & key;
}

void InputState::setKey(short key) {
  keys |= key;
}

void InputState::unsetKey(short key) {
  keys ^= key;
}

void BasicObject::updatePosition() {
  position = position + velocity;
}
