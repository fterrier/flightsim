#include "plane.h"
#include "simulation.h"
#include "gtest/gtest.h"
#include <memory>

using namespace std;

TEST(Simulation, UpdaterCalled) {
  fs::Simulation sim = fs::Simulation();
  shared_ptr<fs::Plane> plane(new fs::Plane());

  fs::Vector3 actual(-1,-1,-1);

  fs::updater updater = [&actual](fs::Vector3 pos) {
    actual = pos;
  };

  sim.addPlane(plane, updater);

  sim.updateSimulation(0);
  EXPECT_EQ(actual, fs::Vector3(0,0,0));
}
