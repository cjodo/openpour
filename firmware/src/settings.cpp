#include "settings.h"

#include <LittleFS.h>

Settings settings;

static constexpr const char* kPath = "/settings.json";

// Every field that is exposed to the app, except the Wi-Fi credentials.
#define SETTINGS_FIELDS(X)                                                      \
  X(hostname) X(lastRecipe) X(scaleCountsPerGram) X(pumpGpsAtFull)              \
  X(pumpMinDuty) X(pumpLagS) X(thetaStepsPerDeg) X(radialStepsPerMm)           \
  X(invertTheta) X(invertRadial) X(thetaHomeDeg) X(radialHomeMm)                \
  X(thetaMinDeg) X(thetaMaxDeg) X(radialMinMm) X(radialMaxMm) X(centerR)        \
  X(centerThetaDeg) X(parkR) X(parkThetaDeg)

void settingsToJson(JsonObject o) {
#define TO_JSON(f) o[#f] = settings.f;
  SETTINGS_FIELDS(TO_JSON)
#undef TO_JSON
  o["wifiSsid"] = settings.wifiSsid;
}

bool settingsFromJson(JsonObjectConst o) {
#define FROM_JSON(f) \
  if (!o[#f].isNull()) settings.f = o[#f].as<decltype(settings.f)>();
  SETTINGS_FIELDS(FROM_JSON)
#undef FROM_JSON
  bool wifiChanged = false;
  if (!o["wifiSsid"].isNull() && settings.wifiSsid != o["wifiSsid"].as<String>()) {
    settings.wifiSsid = o["wifiSsid"].as<String>();
    wifiChanged = true;
  }
  if (!o["wifiPass"].isNull()) {
    settings.wifiPass = o["wifiPass"].as<String>();
    wifiChanged = true;
  }
  return wifiChanged;
}

void loadSettings() {
  File f = LittleFS.open(kPath, "r");
  if (!f) return;  // first boot: keep defaults
  JsonDocument doc;
  if (deserializeJson(doc, f) == DeserializationError::Ok) {
    settingsFromJson(doc.as<JsonObjectConst>());
    settings.wifiPass = doc["wifiPass"] | "";
  }
  f.close();
}

bool saveSettings() {
  JsonDocument doc;
  settingsToJson(doc.to<JsonObject>());
  doc["wifiPass"] = settings.wifiPass;
  File f = LittleFS.open(kPath, "w");
  if (!f) return false;
  serializeJson(doc, f);
  f.close();
  return true;
}
