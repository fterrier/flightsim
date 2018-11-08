#ifndef FS_PLANE_INCLUDED
#define FS_PLANE_INCLUDED

#include "fs_math.h"
#include "simulation.h"

namespace fs {

struct Controls {
  double throttle;
};

class Plane : public BasicObject {
public:
  void updateControls(const fs::InputState &state, double intervalNs);
  void updateVelocity(double intervalNs);
  Controls* getControls();

private:
  Controls controls;
  double _power = 1;
  double _weight = 1;
  void updatePower();
};

void updatePlaneSimulation(const fs::InputState &inputState, fs::Plane &plane, double intervalNs);

} // namespace fs

#endif
