#pragma once
#include <stdint.h>

// ESP32 DevKit V1 (ESP32-WROOM-32, 30 or 38 pin). See docs/wiring.md.
// GPIO 16/17 are free on WROOM modules; on WROVER (PSRAM) boards move the HX711.

constexpr uint8_t PIN_THETA_STEP = 26;
constexpr uint8_t PIN_THETA_DIR = 25;
constexpr uint8_t PIN_RADIAL_STEP = 33;
constexpr uint8_t PIN_RADIAL_DIR = 32;
constexpr uint8_t PIN_STEPPER_EN = 27;  // shared by both drivers, active low

constexpr uint8_t PIN_THETA_ENDSTOP = 18;   // normally-open microswitch to GND
constexpr uint8_t PIN_RADIAL_ENDSTOP = 19;  // normally-open microswitch to GND

constexpr uint8_t PIN_HX711_DOUT = 16;
constexpr uint8_t PIN_HX711_SCK = 17;

constexpr uint8_t PIN_PUMP_PWM = 23;  // gate of the pump MOSFET module
constexpr uint8_t PIN_TEMP_1WIRE = 4; // DS18B20, 4.7k pull-up to 3V3

constexpr uint8_t PIN_BUTTON = 13;     // momentary to GND
constexpr uint8_t PIN_STATUS_LED = 2;  // on-board LED
