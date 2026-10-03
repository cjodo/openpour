#pragma once
#include <Arduino.h>
#include <ArduinoJson.h>

// Runs a recipe: tare, home, then for each stage pour to the cumulative
// target with the stage's flow rate and pattern, wait, and finally park.
namespace brew {

enum class State { Idle, Preparing, Pouring, Waiting, Paused, Finishing, Done, Error, PumpCal };

bool start(const String& recipeId, String& err);
void pause();
void resume();
void stop();
void calibratePump(String& err);
void update();

State state();
bool active();  // anything but Idle / Done / Error
void status(JsonObject o);

}  // namespace brew
