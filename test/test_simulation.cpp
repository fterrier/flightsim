#include "simulation.h"
#include "gtest/gtest.h"
#include <memory>

using namespace std;

TEST(Behaviour, Gravity) {

  // in 1 second, the object accelerates of 9.81
  fs::Vector3 gravity{0, -9.81, 0};
  fs::Vector3 zero{0, 0, 0};
  EXPECT_EQ(gravity, fs::getGravityVelocityUpdate(zero, 1e9));
}
