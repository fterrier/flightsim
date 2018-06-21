hunter_add_package(GTest)
find_package(GTest REQUIRED)

find_package(Threads REQUIRED)

add_executable(test_flightsim test_engine.cpp test_plane.cpp test_fs_math.cpp test_simulation.cpp)
target_link_libraries(test_flightsim flightsim_lib gtest_main gtest ${CMAKE_THREAD_LIBS_INIT})
target_compile_features(test_flightsim PRIVATE cxx_lambdas)

gtest_add_tests(TARGET test_flightsim)
