#include "fs_math.h"
#include "plane.h"
#include "gtest/gtest.h"


TEST(PlaneTest, StraightTest)
{
  fs::Plane* plane = new fs::Plane();

  EXPECT_EQ(fs::Vector3(0,0,0), plane->getLift());
  EXPECT_EQ(fs::Vector3(1,0,0), plane->getPower());

}



