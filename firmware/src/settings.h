#pragma once
#include <Arduino.h>
#include <ArduinoJson.h>

// Persistent, user-tunable settings. Stored as /settings.json on LittleFS and
// edited from the web app. Defaults match the reference build in docs/BOM.md.
struct Settings {
  String wifiSsid;
  String wifiPass;
  String hostname = "openpour";
  String lastRecipe;

  // Scale (HX711 + bar load cell). Calibrate from the app.
  float scaleCountsPerGram = 420.0f;

  // Pump. pumpGpsAtFull is measured by the pump calibration.
  float pumpGpsAtFull = 6.0f;
  float pumpMinDuty = 0.25f;
  float pumpLagS = 0.6f;  // water in flight + scale latency when stopping

  // Motion. Direct-drive theta (NEMA17, 1/16 microstep), GT2 20T radial belt.
  float thetaStepsPerDeg = 200.0f * 16.0f / 360.0f;
  float radialStepsPerMm = 200.0f * 16.0f / 40.0f;
  bool invertTheta = false;
  bool invertRadial = false;
  float thetaHomeDeg = -75.0f;  // arm angle when the theta endstop trips
  float radialHomeMm = 66.0f;   // carriage radius when the radial endstop trips
  float thetaMinDeg = -78.0f;
  float thetaMaxDeg = 25.0f;  // beyond this the beam can reach the column
  float radialMinMm = 66.0f;
  float radialMaxMm = 175.0f;
  float centerR = 110.0f;  // pose over the dripper centre (set by calibration)
  float centerThetaDeg = 0.0f;
  float parkR = 80.0f;
  float parkThetaDeg = -60.0f;
};

extern Settings settings;

void loadSettings();
bool saveSettings();
void settingsToJson(JsonObject o);  // never includes the Wi-Fi password
// Returns true if Wi-Fi credentials changed (caller should reboot).
bool settingsFromJson(JsonObjectConst o);
