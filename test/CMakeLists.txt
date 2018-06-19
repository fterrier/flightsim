find_package(GTest REQUIRED)
find_package(Threads REQUIRED)

add_executable(test_flightsim test_engine.cpp test_plane.cpp test_fs_math.cpp)
target_link_libraries(test_flightsim flightsim_lib gtest_main gtest ${CMAKE_THREAD_LIBS_INIT})

gtest_add_tests(TARGET test_flightsim)
