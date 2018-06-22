#include "game_loop.h"
#include "simulation.h"

#include <chrono>
#include <memory>
#include <spdlog/spdlog.h>

using namespace std;

namespace fs {

void Game::startLoop() {
  using clock = std::chrono::high_resolution_clock;

  // 60fps
  constexpr std::chrono::nanoseconds timestep(16000000);

  auto console = spdlog::stdout_logger_mt("game_loop");

  while (!done()) {
    auto frame_start = clock::now();
    auto time_now = frame_start;
    std::chrono::duration<float> delta;

    while (time_now - frame_start < timestep) {
      typedef std::chrono::nanoseconds ns;
      ns d = std::chrono::duration_cast<ns>(delta);

      console->info("Updating simulation with interval: {0:d}", d.count());
      update(d.count());

      auto last_update = time_now;
      time_now = clock::now();

      delta = time_now - last_update;
    }

    console->info("Rendering frame");
    render();
  }
}

} // namespace fs
