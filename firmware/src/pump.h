#pragma once
#include <stdint.h>

// 12 V pump on a low-side MOSFET module, PWM speed control.
namespace pump {

	void begin();
	void update();  // ends timed runs
	void set(float duty);  // 0..1, cancels any timed run
	void runFor(float duty, uint32_t ms);
	void off();
	float duty();

}  // namespace pump
