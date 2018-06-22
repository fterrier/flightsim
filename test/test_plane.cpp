#include "fs_math.h"
#include "plane.h"
#include "gtest/gtest.h"


TEST(PlaneModelTest, StraightTest)
{
  fs::PlaneModel* plane = new fs::PlaneModel();

  EXPECT_EQ(fs::Vector3(0,0,0), plane->getLift());
  EXPECT_EQ(fs::Vector3(1,0,0), plane->getThrust(2500));

}


TEST(PlaneModelTest, StraightAndLevelTest)
{
  fs::Plane plane;

  plane.setPower(2500);

  // velocity does not change under conditions for straight and level flight
  EXPECT_EQ(fs::Vector3(1,0,0), plane.getComponent(fs::Vector3(1,0,0), 1e9));
}
