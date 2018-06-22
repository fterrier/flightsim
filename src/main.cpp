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
#include "plane.h"
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
    //    viewer.setCameraManipulator(new osgGA::TrackballManipulator);

    osg::Vec3d eye( 100.0, 0.0, 100.0 );
    osg::Vec3d center( -1.0, 0.0, -1.0 );
    osg::Vec3d up( 0.0, 1.0, 0.0 );

    //viewer.getCamera()->setProjectionMatrix(osg::Matrix::perspective(1, 1, 0.1, 500));
    viewer.getCamera()->setViewMatrixAsLookAt( eye, center, up );
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

    fs::updater updater = [child1](fs::Vector3 pos) {
      child1->setPosition(osg::Vec3(pos.x(), pos.y(), pos.z()));
    };

    shared_ptr<fs::BasicObject> plane = make_shared<fs::BasicObject>();
    shared_ptr<fs::Behaviour> gravity = make_shared<fs::GravityBehaviour>();
    plane->addBehaviour(gravity);
    simulation.addObject(plane, updater);
  }
};

int main(int argc, char **argv) {
  osg::ArgumentParser arguments(&argc, argv);

  FlightSimGame flightSimGame;
  flightSimGame.startLoop();

  return 0;
}
