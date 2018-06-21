#include <osg/PolygonMode>
#include <osg/PositionAttitudeTransform>
#include <osg/ShapeDrawable>
#include <osgGA/TrackballManipulator>
#include <osgViewer/Viewer>

#include <chrono>
#include <iostream>
#include <memory>

#include "game_loop.h"
#include "simulation.h"
#include "spdlog/spdlog.h"

using namespace std;

osg::ref_ptr<osg::PositionAttitudeTransform>
CreateSubGraph(osg::ref_ptr<osg::Group> root, osg::ref_ptr<osg::Node> model,
               double translation) {

  osg::ref_ptr<osg::PositionAttitudeTransform> pat(
      new osg::PositionAttitudeTransform());

  root->addChild(pat);
  pat->addChild(model);
  pat->setPosition(osg::Vec3(translation, 0.0, 0.0));

  return pat;
}

void update(osg::ref_ptr<osg::PositionAttitudeTransform> pat) {
  osg::Vec3 pos = pat->getPosition();
  pat->setPosition(osg::Vec3(pos.x() + 0.0000001, pos.y(), pos.z()));
}

class FlightSimGame : public fs::Game {

public:
  FlightSimGame() {
    addObjects();
    initViewer();
  }

  bool done() { return viewer.done(); }

  void update(double interval) { simulation.updateSimulation(interval); }

  void render() { viewer.frame(); }

private:
  fs::Simulation simulation;
  osgViewer::Viewer viewer;
  osg::ref_ptr<osg::Group> root = (new osg::Group);

  void initViewer() {
    // Create a viewer, use it to view the model
    viewer.setSceneData(root);
    viewer.setCameraManipulator(new osgGA::TrackballManipulator);
    viewer.realize();
  }

  void addObjects() {
    osg::ref_ptr<osg::StateSet> ss = root->getOrCreateStateSet();

    osg::ref_ptr<osg::PolygonMode> polyMode(new osg::PolygonMode());
    polyMode->setMode(osg::PolygonMode::FRONT_AND_BACK, osg::PolygonMode::LINE);
    ss->setAttribute(polyMode);

    osg::ref_ptr<osg::Sphere> sphere =
        (new osg::Sphere(osg::Vec3(0, 0, 0), 1.0));
    osg::ref_ptr<osg::ShapeDrawable> drawableSphere =
        (new osg::ShapeDrawable(sphere));

    osg::ref_ptr<osg::PositionAttitudeTransform> child1 =
        CreateSubGraph(root, drawableSphere, 0.0);

    fs::updater updater = [child1](fs::Vector3 pos) { ::update(child1); };
    simulation.addPlane(make_shared<fs::Plane>(), updater);
  }
};

int main(int argc, char **argv) {
  osg::ArgumentParser arguments(&argc, argv);

  FlightSimGame flightSimGame;
  flightSimGame.startLoop();

  return 0;
}
