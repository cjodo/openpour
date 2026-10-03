#pragma once

// HX711 load cell under the dripper platform. Non-blocking: update() reads a
// sample whenever the HX711 has one (10 or 80 SPS depending on the board).
namespace scale {

void begin();
void update();

float grams();    // filtered weight since the last tare
float flowGps();  // grams/second over the last ~1 s
bool flowValid(); // enough history for flowGps() to mean something
bool connected(); // a sample arrived recently

void tare();                     // async; see busy()
void calibrate(float knownGrams);  // async; known weight must be on the platform
bool busy();

}  // namespace scale
