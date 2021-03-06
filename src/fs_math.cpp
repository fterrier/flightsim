#include "fs_math.h"

Vector3 Vector3::operator-(const Vector3 &rhs) const {
  return Vector3{x - rhs.x, y - rhs.y, z - rhs.z};
}

Vector3 Vector3::operator-() const {
  return Vector3{-x, -y, -z};
}

Vector3 Vector3::operator+(const Vector3 &rhs) const {
  return Vector3{x + rhs.x, y + rhs.y, z + rhs.z};
}

Vector3 Vector3::operator*(const double &rhs) const {
  return Vector3{x * rhs, y * rhs, z * rhs};
}

bool Vector3::operator==(const Vector3 &rhs) const {
  return rhs.x == x && rhs.y == y && rhs.z == z;
}

bool Vector3::operator!=(const Vector3 &rhs) const {
  return rhs.x != x || rhs.y != y || rhs.z != z;
}

std::ostream &operator<<(std::ostream &strm, const Vector3 &rhs) {
  strm << "(" << rhs.x << "," << rhs.y << "," << rhs.z << ")";
  return strm;
}
