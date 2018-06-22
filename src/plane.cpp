#include "plane.h"

namespace fs {

  Vector3 PlaneModel::getLift() {
    return Vector3(0,0,0);
  }

  Vector3 PlaneModel::getThrust(double power) {
    return Vector3(1,0,0);
  }

  Plane::Plane(PlaneModel model) {
    _model = model;
  }

  Plane::Plane() {}

  void Plane::setPower(double power) {
    _power = power;
  }

  Vector3 Plane::getComponent(const Vector3& velocity, double intervalNs) {
    return velocity;
  }

}
