#ifndef FS_PLANE_INCLUDED
#define FS_PLANE_INCLUDED

#include "fs_math.h"

namespace fs {

class Plane {

public:
  float getWeight();
  fs::Vector3 getLift();
  fs::Vector3 getDrag();
  fs::Vector3 getPower();
  fs::Moment getMoment();
};

} // namespace fs

#endif
