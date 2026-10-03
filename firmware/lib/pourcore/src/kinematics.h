#pragma once
// Polar-arm kinematics. Pure math, no Arduino dependencies, so it is unit
// tested on the host (see test/test_core).
//
// The arm pivots about a vertical axis. The nozzle carriage slides along the
// arm. A machine pose is therefore (r, theta):
//   r        distance from the pivot axis to the nozzle, mm
//   thetaDeg arm angle about the pivot, degrees
//
// Recipes describe pours in bed coordinates, centred on the dripper:
//   rho      distance from the dripper centre, mm
//   phi      angle around the dripper centre, radians
//
// `centre` is the machine pose that puts the nozzle over the dripper centre.
// It is measured once with the jog/calibrate screen, which absorbs any
// mechanical offset in the printed parts.

#include <cmath>

namespace pour {

constexpr float kPi = 3.14159265358979f;

inline float deg2rad(float d) { return d * kPi / 180.0f; }
inline float rad2deg(float r) { return r * 180.0f / kPi; }

struct ArmPose {
  float r;
  float thetaDeg;
};

struct BedPoint {
  float rho;
  float phi;
};

inline ArmPose bedToArm(const BedPoint& p, const ArmPose& centre) {
  const float x = centre.r + p.rho * std::cos(p.phi);
  const float y = p.rho * std::sin(p.phi);
  return {std::sqrt(x * x + y * y), centre.thetaDeg + rad2deg(std::atan2(y, x))};
}

inline BedPoint armToBed(const ArmPose& a, const ArmPose& centre) {
  const float t = deg2rad(a.thetaDeg - centre.thetaDeg);
  const float x = a.r * std::cos(t) - centre.r;
  const float y = a.r * std::sin(t);
  return {std::sqrt(x * x + y * y), std::atan2(y, x)};
}

}  // namespace pour
