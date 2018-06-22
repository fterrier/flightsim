#ifndef FS_SIMULATION_INCLUDED
#define FS_SIMULATION_INCLUDED

#include "fs_math.h"
#include <functional>
#include <list>
#include <map>
#include <memory>

using namespace std;

namespace fs {

typedef function<void(fs::Vector3)> updater;

class Behaviour {

public:
  virtual Vector3 getComponent(const Vector3& velocity, double intervalNs) = 0;
};

class GravityBehaviour : public Behaviour {

private:
  Vector3 gravityVector = Vector3(0, -1.0F, 0);
  double gravity = 9.81;

public:
  Vector3 getComponent(const Vector3& velocity, double intervalNs) {
    return velocity + gravityVector * gravity * intervalNs * 1e-9;
  }

};

class BasicObject {

public:
  void addBehaviour(shared_ptr<fs::Behaviour> behaviour);

  void updateVelocity(double intervalNs);
  void updatePosition();

  Vector3 getVelocity() { return _velocity; }
  Vector3 getPosition() { return _position; }

protected:
  std::list<shared_ptr<fs::Behaviour>> _behaviours;
  Vector3 _velocity;
  Vector3 _position;
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
