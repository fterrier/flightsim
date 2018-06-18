#
# add OpenSceneGraph
#
message(STATUS "Installing OpenSceneGraph via submodule")
execute_process(COMMAND git submodule update --init -- external/OpenSceneGraph
                WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
add_subdirectory(external/OpenSceneGraph)
