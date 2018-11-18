#include <osg/PolygonMode>
#include <osg/PositionAttitudeTransform>
#include <osg/ShapeDrawable>
#include <osgGA/TrackballManipulator>
#include <osgViewer/Viewer>
#include <osgViewer/ViewerEventHandlers>
#include <osgText/Text>

#include <chrono>
#include <iostream>
#include <memory>
#include <string>

#include "game_loop.h"
#include "plane.h"
#include "simulation.h"
#include "spdlog/spdlog.h"

using namespace std;

static osg::ref_ptr<osg::PositionAttitudeTransform> CreateSubGraph(osg::ref_ptr<osg::Group> root, osg::ref_ptr<osg::Node> model, double translation) {

  osg::ref_ptr<osg::PositionAttitudeTransform> pat(new osg::PositionAttitudeTransform());

  root->addChild(pat);
  pat->addChild(model);
  pat->setPosition(osg::Vec3(translation, 0.0, 0.0));

  return pat;
}

class KeyboardHandler : public osgGA::GUIEventHandler {

private:
  InputState &_inputState;
  // shared_ptr<spdlog::logger> console;

public:
  KeyboardHandler(InputState &inputState) : _inputState(inputState) {
    // console = spdlog::stdout_logger_mt("keyboard");
  }

  virtual bool handle(const osgGA::GUIEventAdapter &ea,
                      osgGA::GUIActionAdapter &aa, osg::Object *obj,
                      osg::NodeVisitor *nv) {

    switch (ea.getEventType()) {

    // TODO replace with key mapping
    case osgGA::GUIEventAdapter::KEYDOWN: {
      if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Up) {
        _inputState.setKey(InputState::KEY_UP);
      }
      if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Down) {
        _inputState.setKey(InputState::KEY_DOWN);
      }

      return false;
    }
    case osgGA::GUIEventAdapter::KEYUP: {
      if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Up) {
        _inputState.unsetKey(InputState::KEY_UP);
      }
      if (ea.getKey() == osgGA::GUIEventAdapter::KEY_Down) {
        _inputState.unsetKey(InputState::KEY_DOWN);
      }

      return false;
    }
    default:
      return false;
    }
  }
};

class FlightSimGame : public Game {

public:
  FlightSimGame() {
    addObjects();
    addDebugInformation();
    initViewer();
  }

  bool done() { return viewer.done(); }

  void update(double interval) {
    updatePlaneSimulation(inputState, plane, interval);
  }

  void render() {
    // TODO use renderSimulation(...);
    // TODO 1. collect all positions
    // TODO 2. collect all render objects that need to be updated
    // update all

    // use updater to update rendering of plane
    Vector3 position = plane.position;
    child->setPosition(osg::Vec3d(position.x, position.y, position.z));

    // we update the rendering of the debug information
    throttleDebug->setText(fmt::format("Throttle: {:-f}\nX: {:-f}\nY: {:-f}\nZ: {:-f}",
                                       plane.getControls()->throttle,
                                       plane.position.x,
                                       plane.position.y,
                                       plane.position.z));

    viewer.frame();
  }

private:
  InputState inputState;
  Plane plane;

  osg::ref_ptr<osg::PositionAttitudeTransform> child;

  // TODO replace with own transform
  osg::ref_ptr<osgText::Text> throttleDebug;

  osgViewer::Viewer viewer;
  osg::ref_ptr<osg::Group> root = (new osg::Group);

  void initViewer() {

    viewer.setSceneData(root);
    // viewer.setCameraManipulator(new osgGA::TrackballManipulator);

    osg::Vec3d eye(0.0, 0.0, -50.0);
    osg::Vec3d center(0.0, 0.0, 0.0);
    osg::Vec3d up(0.0, 1.0, 0.0);

    double height = osg::DisplaySettings::instance()->getScreenHeight();
    double width = osg::DisplaySettings::instance()->getScreenWidth();

    viewer.getCamera()->setViewMatrixAsLookAt(eye, center, up);
    //viewer.getCamera()->setProjectionMatrix(
    //  osg::Matrix::perspective(0.785398f, width / height, 0.1f, 100.0f)
    //);

    viewer.addEventHandler(new KeyboardHandler(inputState));
    viewer.addEventHandler(new osgViewer::StatsHandler);

    //viewer.realize();
  }

  // TODO create own class for stats display
  osg::Camera* createHUDCamera( double left, double right, double bottom, double top )
  {
    osg::ref_ptr<osg::Camera> camera = new osg::Camera;
    camera->setReferenceFrame( osg::Transform::ABSOLUTE_RF );
    camera->setClearMask( GL_DEPTH_BUFFER_BIT );
    camera->setRenderOrder( osg::Camera::POST_RENDER );
    camera->setAllowEventFocus( false );
    camera->setProjectionMatrix(osg::Matrix::ortho2D(left, right, bottom, top));
    return camera.release();
  }

  osg::ref_ptr<osgText::Text> createText(const osg::Vec3& pos, float size)
  {
    osg::ref_ptr<osgText::Font> g_font = osgText::readFontFile("fonts/arial.ttf");

    osg::ref_ptr<osgText::Text> text = new osgText::Text;
    text->setFont(g_font);
    text->setCharacterSize( size );
    text->setAxisAlignment( osgText::TextBase::XY_PLANE );
    text->setText("");
    text->setPosition( pos );
    return text;
  }

  void addObjects() {
    //osg::ref_ptr<osg::StateSet> ss = root->getOrCreateStateSet();

    //osg::ref_ptr<osg::PolygonMode> polyMode(new osg::PolygonMode());
    //polyMode->setMode(osg::PolygonMode::FRONT_AND_BACK, osg::PolygonMode::LINE);
    //ss->setAttribute(polyMode);

    osg::ref_ptr<osg::Sphere> sphere = (new osg::Sphere(osg::Vec3(0, 0, 0), 5.0));
    osg::ref_ptr<osg::ShapeDrawable> drawableSphere = (new osg::ShapeDrawable(sphere));

    child = CreateSubGraph(root, drawableSphere, 0.0);
  }

  void addDebugInformation() {
    osg::ref_ptr<osg::Geode> textGeode = new osg::Geode;

    throttleDebug = createText(osg::Vec3(0.0f, 500.0f, 0.0f), 5.0f);
    textGeode->addDrawable(throttleDebug);
    osg::Camera* camera = createHUDCamera(0, 1024, 0, 512);

    camera->addChild(textGeode);
    camera->getOrCreateStateSet()->setMode(GL_LIGHTING, osg::StateAttribute::OFF);

    root->addChild(camera);
  }

};

int main(int argc, char **argv) {
  osg::ArgumentParser arguments(&argc, argv);

  FlightSimGame flightSimGame;
  flightSimGame.startLoop();

  return 0;
}
