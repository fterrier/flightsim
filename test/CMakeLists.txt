find_package(GTest REQUIRED)

add_executable(test_engine test_engine.cpp)
target_link_libraries(test_engine engine gtest_main gtest)

gtest_add_tests(TARGET test_engine)
