#include "fs_math.h"
#include "plane.h"
#include "gtest/gtest.h"


// TEST(PlaneModelTest, StraightTest)
// {
//   fs::PlaneModel* plane = new fs::PlaneModel();

//   fs::Vector3 zero{0,0,0};
//   fs::Vector3 forward{1,0,0};
//   EXPECT_EQ(zero, plane->getLift());
//   EXPECT_EQ(forward, plane->getThrust(2500));
// }


TEST(PlaneModelTest, StraightAndLevelTest)
{
  fs::Plane plane;

  // parameters
  //plane.setPower(2500);

  fs::Vector3 forward{1,0,0};
  // velocity does not change under conditions for straight and level flight
  updatePlaneSimulation(fs::InputState(), plane, 1e9);

  EXPECT_EQ(forward, plane.velocity);
}


