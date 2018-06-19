#ifndef FS_MATH_INCLUDED
#define FS_MATH_INCLUDED

namespace fs {
class Vector3 {
public:
  Vector3(double x, double y, double z);

  double x();
  double y();
  double z();

  bool operator==(const Vector3 & rhs) const;
  bool operator!=(const Vector3 & rhs) const;

private:
  double _x;
  double _y;
  double _z;
};

class Moment {
public:
  Vector3 moment();
  double arm();
};
} // namespace fs

#endif
