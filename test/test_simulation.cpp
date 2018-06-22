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

TEST(BasicObject, Gravity) {
  fs::BasicObject os = fs::BasicObject();

  // in 1 second, the object accelerates of 9.81
  os.updateVelocity(1e9);
  EXPECT_EQ(fs::Vector3(0,-9.81,0), os.getVelocity());
}
