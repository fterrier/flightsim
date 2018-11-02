cmake_minimum_required(VERSION 3.9)

include("external/HunterGate.cmake")
HunterGate(
    URL "https://github.com/ruslo/hunter/archive/v0.22.20.tar.gz"
    SHA1 "5890f0e022f7147549560ec06f0b47968bff8372"
    )

project(flightsim)

include(GNUInstallDirs)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_BINDIR})

## this seems to be taking too long to build
#include(external/OpenSceneGraph.cmake)

# move to source

hunter_add_package(spdlog)
find_package(spdlog CONFIG REQUIRED)

# hunter_add_package(SDL2)
# find_package(SDL2 CONFIG REQUIRED)

find_package(OpenSceneGraph REQUIRED COMPONENTS osgDB osgGA osgUtil osgViewer)

add_executable(flightsim src/main.cpp)

add_library(flightsim_lib
  src/game_loop.h
  src/game_loop.cpp
  src/plane.h
  src/plane.cpp
  src/simulation.h
  src/simulation.cpp
  src/engine.h
  src/engine.cpp
  src/fs_math.h
  src/fs_math.cpp)

target_compile_features(flightsim_lib PRIVATE cxx_lambdas)
target_compile_features(flightsim PRIVATE cxx_lambdas)

target_include_directories(flightsim_lib PUBLIC src/)

target_link_libraries(flightsim_lib PUBLIC spdlog::spdlog
  #SDL2::SDL2
  )
target_link_libraries(flightsim PUBLIC
  flightsim_lib

  # if using include
  #OpenSceneGraph
  #OpenThreads
  #osgDB osgGA osgUtil osgViewer
  # if using find_package
  ${OPENSCENEGRAPH_LIBRARIES}
  ${OPENTHREADS_LIBRARIES}
  ${OSG_LIBRARIES}
  )
# end

enable_testing()
add_subdirectory(test)
