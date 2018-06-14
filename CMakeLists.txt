cmake_minimum_required(VERSION 3.9)

project(flightsim)
add_executable(flightsim src/main.cpp)

set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

add_library(engine src/engine.h src/engine.cpp)
target_include_directories(engine PUBLIC src/)

target_link_libraries(flightsim PUBLIC engine)

include(cmake/googletest.cmake)
fetch_googletest(
    ${PROJECT_SOURCE_DIR}/cmake
    ${PROJECT_BINARY_DIR}/googletest
    )

enable_testing()
add_subdirectory(test)
