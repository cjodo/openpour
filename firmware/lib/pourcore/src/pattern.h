#pragma once
// Pour patterns as a function of time, in bed (dripper-centred) coordinates.

#include <cmath>
#include <cstring>

#include "kinematics.h"

namespace pour {

enum class Pattern : unsigned char { Center, Circle, Spiral };

// Revolutions taken to sweep from the centre out to the full radius (and the
// same again to come back in).
constexpr float kSpiralTurns = 3.0f;

struct PatternParams {
  Pattern type = Pattern::Center;
  float radiusMm = 0.0f;
  float rps = 1.0f;  // revolutions per second around the dripper centre
};

inline BedPoint patternAt(const PatternParams& p, float t) {
  const float phi = 2.0f * kPi * p.rps * t;
  switch (p.type) {
    case Pattern::Circle:
      return {p.radiusMm, phi};
    case Pattern::Spiral: {
      const float sweep = kSpiralTurns / std::fmax(p.rps, 0.05f);
      const float u = std::fmod(t, 2.0f * sweep) / sweep;  // 0..2
      const float frac = u <= 1.0f ? u : 2.0f - u;          // out, then back in
      return {p.radiusMm * frac, phi};
    }
    case Pattern::Center:
    default:
      return {0.0f, 0.0f};
  }
}

inline Pattern parsePattern(const char* s) {
  if (s && std::strcmp(s, "circle") == 0) return Pattern::Circle;
  if (s && std::strcmp(s, "spiral") == 0) return Pattern::Spiral;
  return Pattern::Center;
}

inline const char* patternName(Pattern p) {
  switch (p) {
    case Pattern::Circle: return "circle";
    case Pattern::Spiral: return "spiral";
    default: return "center";
  }
}

}  // namespace pour
