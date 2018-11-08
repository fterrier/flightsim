#ifndef FS_MATH_INCLUDED
#define FS_MATH_INCLUDED

#include <iostream>

namespace fs {
class Vector3 {
public:
  double x;
  double y;
  double z;

  bool operator==(const Vector3 &rhs) const;
  bool operator!=(const Vector3 &rhs) const;
  Vector3 operator+(const Vector3 &rhs) const;
  Vector3 operator-(const Vector3 &rhs) const;
  Vector3 operator*(const double &rhs) const;
  Vector3 operator-() const;

  friend std::ostream &operator<<(std::ostream &strm, const Vector3 &rhs);
};

class Moment {
public:
  Vector3 moment();
  double arm();
};
} // namespace fs

#endif
