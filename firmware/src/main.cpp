// OpenPour firmware: automated pour-over on an ESP32.
// See ../README.md for the hardware and ../docs for wiring and calibration.

#include <Arduino.h>
#include <ArduinoJson.h>
#include <LittleFS.h>

#include "brew.h"
#include "motion.h"
#include "net.h"
#include "pins.h"
#include "pump.h"
#include "recipes.h"
#include "scale.h"
#include "settings.h"
#include "thermo.h"

static constexpr uint32_t kStatusPeriodMs = 200;
static constexpr uint32_t kLongPressMs = 1500;

static uint32_t lastStatusMs = 0;
static bool rebootPending = false;
static uint32_t rebootAtMs = 0;

static void notify(const char* type, const String& msg) {
  JsonDocument doc;
  doc["t"] = type;
  doc["msg"] = msg;
  String out;
  serializeJson(doc, out);
  net::broadcast(out);
}

static void sendStatus() {
  JsonDocument doc;
  brew::status(doc.to<JsonObject>());
  String out;
  serializeJson(doc, out);
  net::broadcast(out);
}

static void startBrew(const String& id) {
  String err;
  if (!brew::start(id, err)) notify("error", err);
}

static bool idle() { return !brew::active(); }

static void handleCommand(const String& line) {
  JsonDocument doc;
  if (deserializeJson(doc, line)) return;
  const String cmd = doc["cmd"] | "";
  String err;

  if (cmd == "start") {
    startBrew(doc["recipe"] | "");
  } else if (cmd == "pause") {
    brew::pause();
  } else if (cmd == "resume") {
    brew::resume();
  } else if (cmd == "stop") {
    pump::off();
    brew::stop();
  } else if (!idle()) {
    notify("error", "That isn't available while brewing.");
  } else if (cmd == "tare") {
    scale::tare();
  } else if (cmd == "home") {
    motion::home();
  } else if (cmd == "park") {
    motion::park();
  } else if (cmd == "center") {
    motion::moveToBed({0, 0});
  } else if (cmd == "jog") {
    motion::jog(doc["dr"] | 0.0f, doc["dtheta"] | 0.0f);
  } else if (cmd == "setCenter") {
    motion::setCenterHere();
    notify("info", "Dripper centre saved.");
  } else if (cmd == "release") {
    motion::release();
  } else if (cmd == "prime") {
    const float seconds = constrain(doc["seconds"] | 3.0f, 0.0f, 30.0f);
    pump::runFor(constrain(doc["duty"] | 1.0f, 0.0f, 1.0f), (uint32_t)(seconds * 1000));
  } else if (cmd == "calScale") {
    const float grams = doc["grams"] | 0.0f;
    if (grams > 0) {
      scale::calibrate(grams);
      notify("info", "Calibrating the scale. Keep the weight still.");
    }
  } else if (cmd == "calPump") {
    brew::calibratePump(err);
    if (err.length()) notify("error", err);
  } else if (cmd == "settings") {
    if (settingsFromJson(doc["data"].as<JsonObjectConst>())) {
      rebootPending = true;
      rebootAtMs = millis() + 1500;
      notify("info", "Wi-Fi saved. Restarting to connect.");
    } else {
      notify("info", "Settings saved.");
    }
    saveSettings();
  } else if (cmd == "saveRecipes") {
    if (recipes::save(doc["data"]))
      notify("recipes", "Recipes saved.");
    else
      notify("error", "Recipes could not be saved.");
  }
}

// Short press: start the last recipe / pause / resume / clear.
// Long press: stop.
static void updateButton() {
  static bool wasDown = false;
  static uint32_t downAtMs = 0;
  static bool longFired = false;
  const bool down = digitalRead(PIN_BUTTON) == LOW;
  const uint32_t now = millis();

  if (down && !wasDown) {
    downAtMs = now;
    longFired = false;
  } else if (down && !longFired && now - downAtMs > kLongPressMs) {
    longFired = true;
    pump::off();
    brew::stop();
  } else if (!down && wasDown && !longFired && now - downAtMs > 30) {
    switch (brew::state()) {
      case brew::State::Idle:
        startBrew(settings.lastRecipe.length() ? settings.lastRecipe : recipes::firstId());
        break;
      case brew::State::Paused:
        brew::resume();
        break;
      case brew::State::Done:
      case brew::State::Error:
        brew::stop();
        break;
      default:
        brew::pause();
    }
  }
  wasDown = down;
}

// Off when idle, on while brewing, slow blink in AP mode, fast blink on error.
static void updateLed() {
  const uint32_t now = millis();
  bool on;
  switch (brew::state()) {
    case brew::State::Error: on = (now / 125) % 2; break;
    case brew::State::Idle:
    case brew::State::Done: on = net::apMode() && (now / 1000) % 2; break;
    default: on = true;
  }
  digitalWrite(PIN_STATUS_LED, on);
}

void setup() {
  Serial.begin(115200);
  pinMode(PIN_BUTTON, INPUT_PULLUP);
  pinMode(PIN_STATUS_LED, OUTPUT);

  pump::begin();  // pump off before anything else
  if (!LittleFS.begin(true)) Serial.println("LittleFS mount failed");
  loadSettings();
  recipes::ensureDefaults();

  scale::begin();
  thermo::begin();
  motion::begin();
  net::begin();
}

void loop() {
  scale::update();
  thermo::update();
  pump::update();
  motion::update();
  brew::update();
  net::update();
  updateButton();
  updateLed();

  String line;
  while (net::popCommand(line)) handleCommand(line);

  const uint32_t now = millis();
  if (net::wantsStatus() || now - lastStatusMs >= kStatusPeriodMs) {
    lastStatusMs = now;
    sendStatus();
  }
  if (rebootPending && (int32_t)(now - rebootAtMs) >= 0) ESP.restart();
}
