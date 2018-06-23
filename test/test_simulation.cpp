#include "simulation.h"
#include "gtest/gtest.h"
#include <memory>

using namespace std;

TEST(Simulation, UpdaterCalled) {
  fs::Simulation sim = fs::Simulation();
  fs::BasicObject object;

  bool called = false;
  bool *_called = &called;

  fs::updater updater = [_called](fs::Vector3 pos) {
                          *_called = true;
                        };

  sim.addObject(&object, updater);

  sim.updateSimulation(0);
  EXPECT_EQ(called, true);
}

TEST(Behaviour, Gravity) {
  fs::GravityBehaviour b;

  // in 1 second, the object accelerates of 9.81
  EXPECT_EQ(fs::Vector3(0, -9.81, 0),
            b.getComponent(fs::Vector3(0, 0, 0), 1e9));
}
