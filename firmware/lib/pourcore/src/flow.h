#pragma once
// Pump flow control. The scale measures what actually landed, so the pump
// does not need to be precise: duty = feed-forward from the calibrated pump
// rate + a slow integral correction from the measured flow.

#include <algorithm>

namespace pour {

struct FlowController {
  float gpsAtFull = 6.0f;  // grams/second at 100% duty (pump calibration)
  float minDuty = 0.25f;   // below this the pump stalls
  float ki = 0.05f;        // duty per gram of accumulated flow error
  float integ = 0.0f;

  void reset() { integ = 0.0f; }

  float update(float targetGps, float measuredGps, bool measuredValid, float dt) {
    if (targetGps <= 0.0f) return 0.0f;
    if (measuredValid) {
      integ += ki * (targetGps - measuredGps) * dt;
      integ = std::min(std::max(integ, -0.5f), 0.5f);
    }
    float d = targetGps / std::max(gpsAtFull, 0.1f) + integ;
    d = std::min(std::max(d, 0.0f), 1.0f);
    return std::max(d, minDuty);
  }
};

// True once the water already in flight (plus scale latency) will reach the
// target, so the pump should stop now.
inline bool shouldStopPour(float grams, float flowGps, float lagS, float targetG) {
  return grams + std::max(flowGps, 0.0f) * lagS >= targetG;
}

}  // namespace pour
