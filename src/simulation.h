#ifndef FS_SIMULATION_INCLUDED
#define FS_SIMULATION_INCLUDED

#include "fs_math.h"
#include "plane.h"
#include <functional>
#include <map>
#include <memory>

using namespace std;

namespace fs {

typedef function<void(fs::Vector3)> updater;

class Simulation {

public:
  void updateSimulation(int ns);
  void addPlane(shared_ptr<fs::Plane> plane, fs::updater &updater);

private:
  map<shared_ptr<fs::Plane>, fs::updater> updaters;
};
} // namespace fs

#endif
