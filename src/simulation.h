#ifndef FS_SIMULATION_INCLUDED
#define FS_SIMULATION_INCLUDED

#include "fs_math.h"
#include <functional>
#include <map>
#include <memory>

using namespace std;

namespace fs {

typedef function<void(fs::Vector3)> updater;

const Vector3 gravityVector = Vector3(0, -1.0F, 0);
const double gravity = 9.81;

class BasicObject {

public:
  void updateVelocity(double intervalNs);

  void updatePosition();

  Vector3 getVelocity() { return velocity; }
  Vector3 getPosition() { return position; }

private:
  Vector3 velocity;
  Vector3 position;
};

class Simulation {

public:
  void updateSimulation(int ns);
  void addObject(shared_ptr<fs::BasicObject> state, fs::updater &updater);

private:
  map<shared_ptr<fs::BasicObject>, fs::updater> updaters;
};

} // namespace fs

#endif
