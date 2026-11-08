#include <Magnum/GL/Buffer.h>
#include <Magnum/GL/DefaultFramebuffer.h>
#include <Magnum/GL/Mesh.h>
#include <Magnum/GL/Renderer.h>
#include <Magnum/MeshTools/Interleave.h>
#include <Magnum/MeshTools/CompressIndices.h>
#include <Magnum/Primitives/Cube.h>
#include <Magnum/Shaders/Phong.h>
#include <Magnum/Trade/MeshData3D.h>
#include <Magnum/Platform/GLContext.h>

#include <GLFW/glfw3.h>

#include <chrono>
#include <iostream>
#include <memory>
#include <string>

#include "game_loop.h"
#include "plane.h"
#include "simulation.h"
#include "spdlog/spdlog.h"

using namespace std;
using namespace Magnum;

// class KeyboardHandler : public osgGA::GUIEventHandler {

// private:
//   InputState &_inputState;
//   // shared_ptr<spdlog::logger> console;

// public:
//   KeyboardHandler(InputState &inputState) : _inputState(inputState) {
//     // console = spdlog::stdout_logger_mt("keyboard");
//   }

//   virtual bool handle(const osgGA::GUIEventAdapter &ea,
//                       osgGA::GUIActionAdapter &aa, osg::Object *obj,
//                       osg::NodeVisitor *nv) {

//     switch (ea.getEventType()) {

//     // TODO replace with key mapping
//     case osgGA::GUIEventAdapter::KEYDOWN: {
//       if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Up) {
//         _inputState.setKey(InputState::KEY_UP);
//       }
//       if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Down) {
//         _inputState.setKey(InputState::KEY_DOWN);
//       }

//       return false;
//     }
//     case osgGA::GUIEventAdapter::KEYUP: {
//       if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Up) {
//         _inputState.unsetKey(InputState::KEY_UP);
//       }
//       if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Down) {
//         _inputState.unsetKey(InputState::KEY_DOWN);
//       }

//       return false;
//     }
//     default:
//       return false;
//     }
//   }
// };

class FlightSimGame : public Game {

public:
  FlightSimGame() {
    addObjects();
    addDebugInformation();
    initViewer();
  }

  bool done() { 
    // return viewer.done();
    return false;
  }

  void update(double interval) {
    updatePlaneSimulation(inputState, plane, interval);
  }

  void render() {
    // TODO use renderSimulation(...);
    // TODO 1. collect all positions
    // TODO 2. collect all render objects that need to be updated
    // update all

    // use updater to update rendering of plane
    fs::Vector3 position = plane.position;

    // child->setPosition(osg::Vec3d(position.x, position.y, position.z));

    // we update the rendering of the debug information
    // throttleDebug->setText(fmt::format("Throttle: {:-f}\nX: {:-f}\nY: {:-f}\nZ: {:-f}",
    //                                    plane.getControls()->throttle,
    //                                    plane.position.x,
    //                                    plane.position.y,
    //                                    plane.position.z));

    // viewer.frame();
  }

private:
  InputState inputState;
  Plane plane;

  void initViewer() {

  }

  void addObjects() {

  }

  void addDebugInformation() {
    
  }

};

int main(int argc, char **argv) {

  const int width = 800;
  const int height = 600;

  /* Initialize the library */
  if(!glfwInit()) return -1;

  /* Create a windowed mode window and its OpenGL context */
  GLFWwindow* const window = glfwCreateWindow(width, height, "Magnum Plain GLFW Triangle Example", nullptr, nullptr);
  if(!window) {
      glfwTerminate();
      return -1;
  }
  
  /* Make the window's context current */
  glfwMakeContextCurrent(window);

  {
      /* Create Magnum context in an isolated scope */
      Platform::GLContext ctx{argc, argv};

      GL::Buffer _indexBuffer, _vertexBuffer;
      GL::Mesh _mesh;
      Shaders::Phong _shader;

      Matrix4 _transformation, _projection;
      Color3 _color;
      
      // start 
      GL::Renderer::enable(GL::Renderer::Feature::DepthTest);
      GL::Renderer::enable(GL::Renderer::Feature::FaceCulling);

      const Trade::MeshData3D cube = Primitives::cubeSolid();

      _vertexBuffer.setData(MeshTools::interleave(cube.positions(0), cube.normals(0)));

      Containers::Array<char> indexData;
      MeshIndexType indexType;
      UnsignedInt indexStart, indexEnd;
      std::tie(indexData, indexType, indexStart, indexEnd) =
          MeshTools::compressIndices(cube.indices());
      _indexBuffer.setData(indexData);

      _mesh.setPrimitive(cube.primitive())
          .setCount(cube.indices().size())
          .addVertexBuffer(_vertexBuffer, 0, Shaders::Phong::Position{}, Shaders::Phong::Normal{})
          .setIndexBuffer(_indexBuffer, 0, indexType, indexStart, indexEnd);

      _transformation =
          Matrix4::rotationX(30.0_degf)*Matrix4::rotationY(40.0_degf);
      _projection =
          Matrix4::perspectiveProjection(
              35.0_degf, Vector2{width, height}.aspectRatio(), 0.01f, 100.0f)*
          Matrix4::translation(Vector3::zAxis(-10.0f));
      _color = Color3::fromHsv(35.0_degf, 1.0f, 1.0f);

      /* Loop until the user closes the window */
      while(!glfwWindowShouldClose(window)) {

        GL::defaultFramebuffer.clear(GL::FramebufferClear::Color|GL::FramebufferClear::Depth);

        _shader.setLightPosition({7.0f, 5.0f, 2.5f})
            .setLightColor(Color3{1.0f})
            .setDiffuseColor(_color)
            .setAmbientColor(Color3::fromHsv(_color.hue(), 1.0f, 0.3f))
            .setTransformationMatrix(_transformation)
            .setNormalMatrix(_transformation.rotationScaling())
            .setProjectionMatrix(_projection);

        _mesh.draw(_shader);

        
        /* Swap front and back buffers */
        glfwSwapBuffers(window);

        /* Poll for and process events */
        glfwPollEvents();
      }
  }

  glfwTerminate();
  
  // FlightSimGame flightSimGame;
  // flightSimGame.startLoop();

  return 0;
}
