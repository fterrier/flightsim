message(STATUS "Installing spdlog via submodule")
execute_process(COMMAND git submodule update --init -- external/spdlog
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
add_subdirectory(external/spdlog)
