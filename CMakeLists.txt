cmake_minimum_required(VERSION 3.9)

project(flightsim)
add_executable(flightsim src/main.cpp)

include(GNUInstallDirs)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR})
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_BINDIR})

# move to source
add_library(engine src/engine.h src/engine.cpp)
target_include_directories(engine PUBLIC src/)

target_link_libraries(flightsim PUBLIC engine)
# end

enable_testing()
add_subdirectory(test)
