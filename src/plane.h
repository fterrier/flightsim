#ifndef FS_PLANE_INCLUDED
#define FS_PLANE_INCLUDED

#include "fs_math.h"
#include "simulation.h"

struct Controls {
  double throttle;
};

class Plane : public BasicObject {
public:
  void updateControls(const InputState &state, double intervalNs);
  void updateVelocity(double intervalNs);
  Controls* getControls();
  
private:
  // plane state
  Controls controls;
  double _power = 1;

  void updatePower();
  double calculateLiftCoefficient();
  Vector3 calculateLift(Vector3 velocity);

  // plane physical properties
  double _weight = 1;
  double _wingArea = 14.9; // m2
};

void updatePlaneSimulation(const InputState &inputState, Plane &plane, double intervalNs);

#endif
