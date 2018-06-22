#ifndef FS_PLANE_INCLUDED
#define FS_PLANE_INCLUDED

#include "fs_math.h"
#include "simulation.h"

namespace fs {

class PlaneModel {

public:
  PlaneModel() {};

  float getWeight();

  fs::Vector3 getLift();
  fs::Vector3 getDrag();
  fs::Vector3 getThrust(double power);
  fs::Moment getMoment();
};

class Plane : public Behaviour {

public:
  Plane();
  Plane(PlaneModel model);

  Vector3 getComponent(const fs::Vector3& velocity, double intervalNs);
  void setPower(const double power);

private:
  PlaneModel _model;
  double _power;

};

} // namespace fs

#endif
