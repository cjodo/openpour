#include "scale.h"

#include <Arduino.h>
#include <HX711.h>

#include "pins.h"
#include "settings.h"

namespace scale {

static HX711 hx;

static long offset = 0;
static float filtered = 0;
static uint32_t lastSampleMs = 0;

enum class Op { None, Tare, Calibrate };
static Op op = Op::None;
static int64_t acc = 0;
static int accN = 0;
static float calGrams = 0;
static constexpr int kAvgSamples = 16;

// Decimated history for flow-rate estimation.
struct Sample {
  uint32_t ms;
  float g;
};
static constexpr int kHist = 40;  // 40 x 50 ms = 2 s
static Sample hist[kHist];
static int histHead = 0;
static int histCount = 0;
static uint32_t lastPushMs = 0;

static void clearHistory() {
  histHead = 0;
  histCount = 0;
}

static void push(uint32_t ms, float g) {
  hist[histHead] = {ms, g};
  histHead = (histHead + 1) % kHist;
  if (histCount < kHist) histCount++;
}

void begin() {
  hx.begin(PIN_HX711_DOUT, PIN_HX711_SCK);
  tare();
}

void update() {
  if (!hx.is_ready()) return;
  const long raw = hx.read();
  const uint32_t now = millis();
  lastSampleMs = now;

  if (op != Op::None) {
    acc += raw;
    if (++accN < kAvgSamples) return;
    const long avg = (long)(acc / accN);
    if (op == Op::Tare) {
      offset = avg;
      filtered = 0;
      clearHistory();
    } else if (calGrams > 0) {
      const float f = (float)(avg - offset) / calGrams;
      if (fabsf(f) > 1.0f) {
        settings.scaleCountsPerGram = f;
        saveSettings();
      }
    }
    op = Op::None;
    return;
  }

  const float g = (float)(raw - offset) / settings.scaleCountsPerGram;
  filtered += 0.35f * (g - filtered);
  if (now - lastPushMs >= 50) {
    lastPushMs = now;
    push(now, filtered);
  }
}

float grams() { return filtered; }

static const Sample* sampleAgo(uint32_t ageMs) {
  if (histCount < 2) return nullptr;
  const Sample& newest = hist[(histHead - 1 + kHist) % kHist];
  for (int i = 2; i <= histCount; i++) {
    const Sample& s = hist[(histHead - i + kHist) % kHist];
    if (newest.ms - s.ms >= ageMs) return &s;
  }
  return nullptr;
}

bool flowValid() { return sampleAgo(1000) != nullptr; }

float flowGps() {
  const Sample* old = sampleAgo(1000);
  if (!old) return 0;
  const Sample& newest = hist[(histHead - 1 + kHist) % kHist];
  return (newest.g - old->g) * 1000.0f / (float)(newest.ms - old->ms);
}

bool connected() { return millis() - lastSampleMs < 500; }

void tare() {
  op = Op::Tare;
  acc = 0;
  accN = 0;
}

void calibrate(float knownGrams) {
  op = Op::Calibrate;
  calGrams = knownGrams;
  acc = 0;
  accN = 0;
}

bool busy() { return op != Op::None; }

}  // namespace scale
