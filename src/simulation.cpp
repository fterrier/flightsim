#include "simulation.h"

using namespace std;

namespace fs {

void Simulation::updateSimulation(int ns) {

  for (map<shared_ptr<fs::Plane>, fs::updater>::iterator it = updaters.begin();
       it != updaters.end(); ++it) {
    fs::updater updater = it->second;

    // TODO work out position

    updater(fs::Vector3(0, 0, 0));
  }
}

void Simulation::addPlane(shared_ptr<fs::Plane> plane, fs::updater &updater) {
  updaters[plane] = updater;
}

} // namespace fs
