find_package(GTest REQUIRED)
find_package(Threads REQUIRED)

add_executable(test_engine test_engine.cpp)
target_link_libraries(test_engine engine gtest_main gtest ${CMAKE_THREAD_LIBS_INIT})

gtest_add_tests(TARGET test_engine)
