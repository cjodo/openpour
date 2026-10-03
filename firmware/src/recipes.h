#pragma once
#include <Arduino.h>
#include <ArduinoJson.h>
#include <pattern.h>

#include <vector>

// Recipes live in /recipes.json as an array; the web app edits the whole
// file. Each stage's `water` is the amount added in that stage (grams).
struct Stage {
  String name;
  float waterG = 0;
  float flowGps = 4;
  pour::PatternParams pattern;
  float waitS = 0;
};

struct Recipe {
  String id;
  String name;
  float minTempC = 0;
  std::vector<Stage> stages;

  float totalWater() const {
    float t = 0;
    for (const auto& s : stages) t += s.waterG;
    return t;
  }
};

namespace recipes {

constexpr const char* kPath = "/recipes.json";

void ensureDefaults();
bool load(const String& id, Recipe& out);
String firstId();
bool save(JsonVariantConst array);

}  // namespace recipes
