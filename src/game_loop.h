#ifndef FS_GAME_LOOP_INCLUDED
#define FS_GAME_LOOP_INCLUDED

#include <iostream>
#include <memory>

using namespace std;

class Game {
public:
  virtual bool done() { return false; };
  virtual void update(double interval){};
  virtual void render(){};

  void startLoop();
};

#endif
