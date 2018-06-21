#include <osg/PolygonMode>
#include <osg/PositionAttitudeTransform>
#include <osg/ShapeDrawable>
#include <osgGA/TrackballManipulator>
#include <osgViewer/Viewer>

#include "simulation.h"
#include <memory>
#include <chrono>
#include <iostream>

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

int main(int argc, char **argv) {
  osg::ArgumentParser arguments(&argc, argv);

  osg::ref_ptr<osg::Group> root = (new osg::Group);

  osg::ref_ptr<osg::Sphere> sphere = (new osg::Sphere(osg::Vec3(0, 0, 0), 1.0));
  osg::ref_ptr<osg::ShapeDrawable> drawableSphere =
      (new osg::ShapeDrawable(sphere));

  osg::ref_ptr<osg::PositionAttitudeTransform> child1 =
      CreateSubGraph(root, drawableSphere, 0.0);

  osg::ref_ptr<osg::StateSet> ss = root->getOrCreateStateSet();

  osg::ref_ptr<osg::PolygonMode> polyMode(new osg::PolygonMode());
  polyMode->setMode(osg::PolygonMode::FRONT_AND_BACK, osg::PolygonMode::LINE);
  ss->setAttribute(polyMode);

  // Create a viewer, use it to view the model
  osgViewer::Viewer viewer;
  viewer.setSceneData(root);

  viewer.setCameraManipulator(new osgGA::TrackballManipulator);

  fs::Simulation sim = fs::Simulation();

  fs::updater updater = [child1](fs::Vector3 pos) { update(child1); };
  sim.addPlane(make_shared<fs::Plane>(), updater);

  viewer.realize();

  using clock = std::chrono::high_resolution_clock;

  // 60fps
  constexpr std::chrono::nanoseconds timestep(16000000);

  auto console = spdlog::stdout_logger_mt("console");

  while (!viewer.done()) {
    // TODO process user input

    auto frame_start = clock::now();
    auto time_now = frame_start;
    std::chrono::duration<float> delta;

    while(time_now - frame_start < timestep) {
      typedef std::chrono::nanoseconds ns;
      ns d = std::chrono::duration_cast<ns>(delta);

      console->info("Updating simulation with interval: {0:d}", d.count());
      sim.updateSimulation(d.count());

      auto last_update = time_now;
      time_now = clock::now();

      delta = time_now - last_update;
    }

    console->info("Rendering frame");
    viewer.frame();
  }
  return 0;
}
