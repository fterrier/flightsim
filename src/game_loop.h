#ifndef FS_GAME_LOOP_INCLUDED
#define FS_GAME_LOOP_INCLUDED

#include <memory>

using namespace std;

namespace fs {

class Game {
public:
  bool done() {return false;};
  void update(double interval) {};
  void render() {};
};

  void startLoop(shared_ptr<Game> game);

} // namespace fs

#endif
