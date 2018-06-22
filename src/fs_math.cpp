#include "fs_math.h"

namespace fs {

Vector3::Vector3() {}

Vector3::Vector3(double x, double y, double z) {
  _x = x;
  _y = y;
  _z = z;
}

double Vector3::x() { return _x; }

double Vector3::y() { return _y; }

double Vector3::z() { return _z; }

fs::Vector3 Vector3::operator+(const fs::Vector3 &rhs) const {
  return fs::Vector3(_x + rhs._x, _y + rhs._y, _z + rhs._z);
}

fs::Vector3 Vector3::operator*(const double &rhs) const {
  return fs::Vector3(_x * rhs, _y * rhs, _z * rhs);
}

bool Vector3::operator==(const fs::Vector3 &rhs) const {
  return rhs._x == _x && rhs._y == _y && rhs._z == _z;
}

bool Vector3::operator!=(const fs::Vector3 &rhs) const {
  return rhs._x != _x || rhs._y != _y || rhs._z != _z;
}

} // namespace fs
