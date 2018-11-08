#ifndef FS_SIMULATION_INCLUDED
#define FS_SIMULATION_INCLUDED

#include "fs_math.h"
#include <functional>
#include <map>
#include <memory>
#include <vector>

using namespace std;

namespace fs {

class InputState {

private:
  unsigned long long keys;

public:
  bool key(short key) const;
  void setKey(short key);
  void unsetKey(short key);

  enum Key { KEY_UP = 1 << 1, KEY_DOWN = 1 << 2 };
};

class BasicObject {

public:
  Vector3 velocity;
  Vector3 position;

  void updatePosition();
};

} // namespace fs

#endif
