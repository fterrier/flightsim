#include <osg/DisplaySettings>
#include <osg/Fog>
#include <osg/Geometry>
#include <osg/LineWidth>
#include <osg/MatrixTransform>
#include <osg/PolygonMode>
#include <osg/PositionAttitudeTransform>
#include <osg/ShapeDrawable>
#include <osgDB/ReadFile>
#include <osgGA/TrackballManipulator>
#include <osgViewer/CompositeViewer>
#include <osgViewer/Viewer>

// Given a Camera, create a wireframe representation of its
// view frustum. Create a default representation if camera==NULL.
osg::Node *makeFrustumFromCamera(osg::Camera *camera) {
  // Projection and ModelView matrices
  osg::Matrixd proj;
  osg::Matrixd mv;
  if (camera) {
    proj = camera->getProjectionMatrix();
    mv = camera->getViewMatrix();
  } else {
    // Create some kind of reasonable default Projection matrix.
    proj.makePerspective(30., 1., 1., 10.);
    // leave mv as identity
  }

  // Get near and far from the Projection matrix.
  const double near = proj(3, 2) / (proj(2, 2) - 1.0);
  const double far = proj(3, 2) / (1.0 + proj(2, 2));

  // Get the sides of the near plane.
  const double nLeft = near * (proj(2, 0) - 1.0) / proj(0, 0);
  const double nRight = near * (1.0 + proj(2, 0)) / proj(0, 0);
  const double nTop = near * (1.0 + proj(2, 1)) / proj(1, 1);
  const double nBottom = near * (proj(2, 1) - 1.0) / proj(1, 1);

  // Get the sides of the far plane.
  const double fLeft = far * (proj(2, 0) - 1.0) / proj(0, 0);
  const double fRight = far * (1.0 + proj(2, 0)) / proj(0, 0);
  const double fTop = far * (1.0 + proj(2, 1)) / proj(1, 1);
  const double fBottom = far * (proj(2, 1) - 1.0) / proj(1, 1);

  // Our vertex array needs only 9 vertices: The origin, and the
  // eight corners of the near and far planes.
  osg::Vec3Array *v = new osg::Vec3Array;
  v->resize(9);
  (*v)[0].set(0., 0., 0.);
  (*v)[1].set(nLeft, nBottom, -near);
  (*v)[2].set(nRight, nBottom, -near);
  (*v)[3].set(nRight, nTop, -near);
  (*v)[4].set(nLeft, nTop, -near);
  (*v)[5].set(fLeft, fBottom, -far);
  (*v)[6].set(fRight, fBottom, -far);
  (*v)[7].set(fRight, fTop, -far);
  (*v)[8].set(fLeft, fTop, -far);

  osg::Geometry *geom = new osg::Geometry;
  geom->setUseDisplayList(false);
  geom->setVertexArray(v);

  osg::Vec4Array *c = new osg::Vec4Array;
  c->push_back(osg::Vec4(1., 1., 1., 1.));
  geom->setColorArray(c, osg::Array::BIND_OVERALL);

  GLushort idxLines[8] = {0, 5, 0, 6, 0, 7, 0, 8};
  GLushort idxLoops0[4] = {1, 2, 3, 4};
  GLushort idxLoops1[4] = {5, 6, 7, 8};
  geom->addPrimitiveSet(
      new osg::DrawElementsUShort(osg::PrimitiveSet::LINES, 8, idxLines));
  geom->addPrimitiveSet(
      new osg::DrawElementsUShort(osg::PrimitiveSet::LINE_LOOP, 4, idxLoops0));
  geom->addPrimitiveSet(
      new osg::DrawElementsUShort(osg::PrimitiveSet::LINE_LOOP, 4, idxLoops1));

  osg::Geode *geode = new osg::Geode;
  geode->addDrawable(geom);

  geode->getOrCreateStateSet()->setMode(
      GL_LIGHTING, osg::StateAttribute::OFF | osg::StateAttribute::PROTECTED);

  // Create parent MatrixTransform to transform the view volume by
  // the inverse ModelView matrix.
  osg::MatrixTransform *mt = new osg::MatrixTransform;
  mt->setMatrix(osg::Matrixd::inverse(mv));
  mt->addChild(geode);

  return mt;
}

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

void update(osg::ref_ptr<osg::PositionAttitudeTransform> pat, float intvl) {
  osg::Vec3 pos = pat->getPosition();
  pat->setPosition(osg::Vec3(pos.x() + intvl, pos.y(), pos.z()));
}

int main(int argc, char **argv) {
  osg::ArgumentParser arguments(&argc, argv);

  osg::ref_ptr<osg::Group> root = (new osg::Group);

  osg::ref_ptr<osg::Sphere> sphere = (new osg::Sphere(osg::Vec3(0, 0, 0), 1.0));
  osg::ref_ptr<osg::ShapeDrawable> drawableSphere =
      (new osg::ShapeDrawable(sphere));

  osg::ref_ptr<osg::PositionAttitudeTransform> child1 =
      CreateSubGraph(root, drawableSphere, 0.0);
  osg::ref_ptr<osg::PositionAttitudeTransform> child2 =
      CreateSubGraph(root, drawableSphere, 10.0);

  const osg::Vec4 fogColor(0.8, 0.1, 0.1, 0.9);

  osg::ref_ptr<osg::StateSet> ss = root->getOrCreateStateSet();
  ss->setMode(GL_LIGHTING, osg::StateAttribute::OFF);

  osg::ref_ptr<osg::PolygonMode> polyMode(new osg::PolygonMode());
  polyMode->setMode(osg::PolygonMode::FRONT_AND_BACK, osg::PolygonMode::LINE);
  ss->setAttribute(polyMode);

  osg::ref_ptr<osg::Fog> fog(new osg::Fog());
  fog->setMode(osg::Fog::LINEAR);
  fog->setColor(fogColor);
  fog->setStart(15.0);
  fog->setEnd(100.0);
  ss->setAttributeAndModes(fog);

  // Create a viewer, use it to view the model
  osgViewer::Viewer viewer;
  viewer.setSceneData(root);

  const osg::Vec4 color(0.3, 0.3, 0.8, 1);
  viewer.getCamera()->setClearColor(color);

  viewer.setCameraManipulator(new osgGA::TrackballManipulator);


  viewer.realize();

  while (!viewer.done()) {
    viewer.frame();
    update(child1, 0.001);
  }

  // osgViewer::CompositeViewer viewer(arguments);
  // // Turn on FSAA, makes the lines look better.
  // osg::DisplaySettings::instance()->setNumMultiSamples(4);

  // // Create View 0 -- Just the loaded model.
  // {
  //   osgViewer::View *view = new osgViewer::View;
  //   viewer.addView(view);

  //   view->setUpViewInWindow(10, 10, 640, 480);
  //   view->setSceneData(scene.get());
  //   view->setCameraManipulator(new osgGA::TrackballManipulator);
  // }

  // // Create view 1 -- Contains the loaded moel, as well as a wireframe
  // frustum
  // // derived from View 0's Camera.
  // {
  //   osgViewer::View *view = new osgViewer::View;
  //   viewer.addView(view);

  //   view->setUpViewInWindow(10, 510, 640, 480);
  //   view->setSceneData(root.get());
  //   view->setCameraManipulator(new osgGA::TrackballManipulator);
  // }

  // while (!viewer.done()) {
  //   // Update the wireframe frustum
  //   root->removeChild(0, 1);
  //   root->insertChild(0,
  //   makeFrustumFromCamera(viewer.getView(0)->getCamera()));

  //   viewer.frame();
  // }
  return 0;
}
