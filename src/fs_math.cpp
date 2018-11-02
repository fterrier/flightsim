#include "fs_math.h"

namespace fs {


fs::Vector3 Vector3::operator+(const fs::Vector3 &rhs) const {
  return fs::Vector3{x + rhs.x, y + rhs.y, z + rhs.z};
}

fs::Vector3 Vector3::operator*(const double &rhs) const {
  return fs::Vector3{x * rhs, y * rhs, z * rhs};
}

bool Vector3::operator==(const fs::Vector3 &rhs) const {
  return rhs.x == x && rhs.y == y && rhs.z == z;
}

bool Vector3::operator!=(const fs::Vector3 &rhs) const {
  return rhs.x != x || rhs.y != y || rhs.z != z;
}

std::ostream &operator<<(std::ostream &strm, const Vector3 &rhs) {
  strm << "(" << rhs.x << "," << rhs.y << "," << rhs.z << ")";
  return strm;
}

} // namespace fs
