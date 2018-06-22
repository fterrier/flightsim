#include "simulation.h"
#include "gtest/gtest.h"
#include <memory>

using namespace std;

TEST(Simulation, UpdaterCalled) {
  fs::Simulation sim = fs::Simulation();
  shared_ptr<fs::BasicObject> object(new fs::BasicObject());

  fs::Vector3 actual(-1,-1,-1);

  fs::updater updater = [&actual](fs::Vector3 pos) {
    actual = pos;
  };

  sim.addObject(object, updater);

  sim.updateSimulation(0);
  EXPECT_EQ(actual, fs::Vector3(0,0,0));
}

TEST(Behaviour, Gravity) {
  shared_ptr<fs::Behaviour> b = make_shared<fs::GravityBehaviour>();

  // in 1 second, the object accelerates of 9.81
  EXPECT_EQ(fs::Vector3(0,-9.81,0), b->getComponent(fs::Vector3(0,0,0), 1e9));
}
