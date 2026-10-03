#include "recipes.h"

#include <LittleFS.h>

#include "generated/web_assets.h"  // kDefaultRecipesJson, from web/default-recipes.json

namespace recipes {

static bool readAll(JsonDocument& doc) {
  File f = LittleFS.open(kPath, "r");
  if (!f) return false;
  const bool ok = deserializeJson(doc, f) == DeserializationError::Ok && doc.is<JsonArray>();
  f.close();
  return ok;
}

void ensureDefaults() {
  JsonDocument doc;
  if (readAll(doc)) return;
  File f = LittleFS.open(kPath, "w");
  if (!f) return;
  f.print(kDefaultRecipesJson);
  f.close();
}

bool load(const String& id, Recipe& out) {
  JsonDocument doc;
  if (!readAll(doc)) return false;
  for (JsonObjectConst r : doc.as<JsonArrayConst>()) {
    if (id != (r["id"] | "")) continue;
    out.id = id;
    out.name = r["name"] | "Untitled";
    out.minTempC = r["minTemp"] | 0.0f;
    out.stages.clear();
    for (JsonObjectConst s : r["stages"].as<JsonArrayConst>()) {
      Stage st;
      st.name = s["name"] | "Pour";
      st.waterG = s["water"] | 0.0f;
      st.flowGps = s["flow"] | 4.0f;
      st.pattern.type = pour::parsePattern(s["pattern"] | "center");
      st.pattern.radiusMm = s["radius"] | 20.0f;
      st.pattern.rps = s["rps"] | 1.0f;
      st.waitS = s["wait"] | 0.0f;
      if (st.waterG > 0 || st.waitS > 0) out.stages.push_back(st);
    }
    return !out.stages.empty();
  }
  return false;
}

String firstId() {
  JsonDocument doc;
  if (!readAll(doc)) return "";
  return doc[0]["id"] | "";
}

bool save(JsonVariantConst array) {
  if (!array.is<JsonArrayConst>()) return false;
  File f = LittleFS.open(kPath, "w");
  if (!f) return false;
  serializeJson(array, f);
  f.close();
  return true;
}

}  // namespace recipes
