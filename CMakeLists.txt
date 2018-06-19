cmake_minimum_required(VERSION 3.9)

project(flightsim)

include(GNUInstallDirs)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_BINDIR})

## this seems to be taking too long to build
#include(external/OpenSceneGraph.cmake)

find_package(OpenSceneGraph REQUIRED COMPONENTS osgDB osgGA osgUtil osgViewer)

# move to source
add_executable(flightsim src/main.cpp)

add_library(flightsim_lib
  src/plane.h
  src/plane.cpp
  src/engine.h
  src/engine.cpp
  src/fs_math.h
  src/fs_math.cpp)

target_include_directories(flightsim_lib PUBLIC src/)

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
