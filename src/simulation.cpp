#include "simulation.h"
#include "fs_math.h"

#include <spdlog/spdlog.h>

using namespace std;

namespace fs {

auto console = spdlog::stdout_logger_mt("simulation");

void BasicObject::addBehaviour(Behaviour* behaviour) {
  _behaviours.push_back(behaviour);
}

void BasicObject::updateVelocity(double intervalNs) {
  for (std::list<fs::Behaviour*>::iterator it = _behaviours.begin();
       it != _behaviours.end(); ++it) {
    _velocity = (*it)->getComponent(_velocity, intervalNs);
  }
}

void BasicObject::updatePosition() { _position = _position + _velocity; }

void Simulation::updateSimulation(int ns) {

  for (map<fs::BasicObject*, fs::updater>::iterator it =
           updaters.begin();
       it != updaters.end(); ++it) {
    fs::BasicObject os = *(it->first);

    // update position
    os.updateVelocity(ns);
    os.updatePosition();

    // use updater to update rendering
    fs::updater updater = it->second;
    updater(os.getPosition() * 1e-6);
  }
}

void Simulation::addObject(fs::BasicObject* state, fs::updater &updater) {
  updaters[state] = updater;
}

} // namespace fs
