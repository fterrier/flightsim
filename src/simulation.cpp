#include "simulation.h"
#include "fs_math.h"

#include <spdlog/spdlog.h>

using namespace std;

namespace fs {

auto console = spdlog::stdout_logger_mt("simulation");

void BasicObject::updateVelocity(double intervalNs) {
  // velocity is current velocity + force
  velocity = velocity + gravityVector * gravity * intervalNs * 1e-9;
}

void BasicObject::updatePosition() { position = position + velocity; }

void Simulation::updateSimulation(int ns) {

  for (map<shared_ptr<fs::BasicObject>, fs::updater>::iterator it =
           updaters.begin();
       it != updaters.end(); ++it) {
    shared_ptr<fs::BasicObject> os = it->first;

    // update position
    os->updateVelocity(ns);
    os->updatePosition();

    // use updater to update rendering
    fs::updater updater = it->second;
    updater(os->getPosition() * 1e-6);
  }
}

void Simulation::addObject(shared_ptr<fs::BasicObject> state,
                           fs::updater &updater) {
  updaters[state] = updater;
}

} // namespace fs
