#include "gtest/gtest.h"
#include "fs_math.h"

TEST(Vector3Test, Equal)
{
  EXPECT_EQ(fs::Vector3(1,0,0), fs::Vector3(1,0,0));
}

TEST(Vector3Test, NotEqual)
{
  EXPECT_NE(fs::Vector3(0,1,0), fs::Vector3(1,0,0));
  EXPECT_NE(fs::Vector3(0,0,1), fs::Vector3(1,0,0));
  EXPECT_NE(fs::Vector3(0,0,1), fs::Vector3(0,1,0));
  EXPECT_NE(fs::Vector3(1,0,0), fs::Vector3(0,1,0));
  EXPECT_NE(fs::Vector3(1,0,0), fs::Vector3(0,0,1));
  EXPECT_NE(fs::Vector3(0,1,0), fs::Vector3(0,0,1));
}

TEST(Vector3Test, Plus)
{
  EXPECT_EQ(fs::Vector3(2,4,6), fs::Vector3(1,2,3) + fs::Vector3(1,2,3));
}

TEST(Vector3Test, ScalarMult)
{
  EXPECT_EQ(fs::Vector3(2,4,6), fs::Vector3(1,2,3) * 2);
}

