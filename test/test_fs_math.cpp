#include "gtest/gtest.h"
#include "fs_math.h"

Vector3 forward{1,0,0};
Vector3 side{0,0,1};
Vector3 up{0,1,0};


TEST(Vector3Test, Equal)
{
  EXPECT_EQ(forward, forward);
}

TEST(Vector3Test, NotEqual)
{
  EXPECT_NE(forward, side);
  EXPECT_NE(forward, up);
  EXPECT_NE(side, up);
}

TEST(Vector3Test, Plus)
{
  Vector3 v1{1,2,3};
  Vector3 v2{2,4,6};
  EXPECT_EQ(v2, v1 + v1);
}

TEST(Vector3Test, ScalarMult)
{
  Vector3 v1{1,2,3};
  Vector3 v2{2,4,6};
  EXPECT_EQ(v2, v1 * 2);
}

