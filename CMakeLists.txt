cmake_minimum_required(VERSION 3.9)

project(flightsim)
add_executable(flightsim src/main.cpp)

include(GNUInstallDirs)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_BINDIR})

## this seems to be taking too long to build
#include(external/OpenSceneGraph.cmake)

find_package(OpenSceneGraph REQUIRED COMPONENTS osgDB osgGA osgUtil osgViewer)

# move to source
add_library(engine src/engine.h src/engine.cpp)
target_include_directories(engine PUBLIC src/)

target_link_libraries(flightsim PUBLIC
  engine

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
