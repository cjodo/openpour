#include "pump.h"

#include <Arduino.h>

#include "pins.h"

namespace pump {
	static constexpr uint32_t kFreqHz = 5000;
	static constexpr uint8_t kBits = 10;
	static constexpr uint32_t kMax = (1u << kBits) - 1;
#if ESP_ARDUINO_VERSION_MAJOR < 3
	static constexpr uint8_t kChannel = 0;
#endif

	static float current = 0;
	static uint32_t stopAtMs = 0;
	static bool timed = false;

	static void write(float d) {
		current = constrain(d, 0.0f, 1.0f);
		const uint32_t v = (uint32_t)(current * kMax + 0.5f);
#if ESP_ARDUINO_VERSION_MAJOR >= 3
		ledcWrite(PIN_PUMP_PWM, v);
#else
		ledcWrite(kChannel, v);
#endif
	}

	void begin() {
#if ESP_ARDUINO_VERSION_MAJOR >= 3
		ledcAttach(PIN_PUMP_PWM, kFreqHz, kBits);
#else
		ledcSetup(kChannel, kFreqHz, kBits);
		ledcAttachPin(PIN_PUMP_PWM, kChannel);
#endif
		write(0);
	}

	void update() {
		if (timed && (int32_t)(millis() - stopAtMs) >= 0) off();
	}

	void set(float duty) {
		timed = false;
		write(duty);
	}

	void runFor(float duty, uint32_t ms) {
		write(duty);
		stopAtMs = millis() + ms;
		timed = true;
	}

	void off() {
		timed = false;
		write(0);
	}

	float duty() { return current; }

}  // namespace pump
