#include "plane.h"

namespace fs {

  
  Vector3 Plane::getLift() {
    return Vector3(0,0,0);
  }

  Vector3 Plane::getPower() {
    return Vector3(1,0,0);
  }

}
