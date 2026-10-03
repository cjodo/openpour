#include "thermo.h"

#include <Arduino.h>
#include <DallasTemperature.h>
#include <OneWire.h>

#include "pins.h"

namespace thermo {

static OneWire oneWire(PIN_TEMP_1WIRE);
static DallasTemperature sensors(&oneWire);
static float lastC = DEVICE_DISCONNECTED_C;
static uint32_t requestedMs = 0;

void begin() {
  sensors.begin();
  sensors.setResolution(11);  // 0.125 C, ~375 ms conversion
  sensors.setWaitForConversion(false);
  sensors.requestTemperatures();
  requestedMs = millis();
}

void update() {
  if (millis() - requestedMs < 800) return;
  lastC = sensors.getTempCByIndex(0);
  sensors.requestTemperatures();
  requestedMs = millis();
}

float celsius() { return lastC; }

bool present() { return lastC != DEVICE_DISCONNECTED_C; }

}  // namespace thermo
