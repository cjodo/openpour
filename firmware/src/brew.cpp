#include "brew.h"

#include <flow.h>

#include "motion.h"
#include "pump.h"
#include "recipes.h"
#include "scale.h"
#include "settings.h"
#include "thermo.h"

namespace brew {

	static constexpr uint32_t kNoFlowMs = 5000;   // pump on this long ...
	static constexpr float kNoFlowMinG = 2.0f;    // ... must add at least this much
	static constexpr float kOverflowMarginG = 60.0f;
	static constexpr uint32_t kFlowSettleMs = 1500;
	static constexpr uint32_t kPumpCalMs = 10000;
	static constexpr uint32_t kPumpCalSettleMs = 2000;

	static State st = State::Idle;
	static State pausedFrom = State::Idle;
	static Recipe recipe;
	static size_t stageIdx = 0;
	static float stageTarget = 0;  // cumulative grams at the end of this stage
	static uint32_t stateStartMs = 0;
	static uint32_t pausedInStateMs = 0;
	static uint32_t lastUpdateMs = 0;
	static uint32_t elapsedMs = 0;
	static bool prepMoved = false;
	static bool finishToIdle = false;
	static uint32_t flowCheckMs = 0;
	static float flowCheckG = 0;
	static pour::FlowController fc;
	static String error;
	static String message;

	enum class CalStep { Prepare, Run, Settle };
	static CalStep calStep = CalStep::Prepare;

	static void enter(State s) {
		st = s;
		stateStartMs = millis();
	}

	static void fail(const String& msg) {
		pump::off();
		motion::hold();
		error = msg;
		enter(State::Error);
	}

	static const Stage& stage() { return recipe.stages[stageIdx]; }

	static void resetFlowWatchdog() {
		flowCheckMs = millis();
		flowCheckG = scale::grams();
	}

	static void beginStage(size_t i) {
		stageIdx = i;
		stageTarget += stage().waterG;
		fc.gpsAtFull = settings.pumpGpsAtFull;
		fc.minDuty = settings.pumpMinDuty;
		fc.reset();
		if (stage().waterG <= 0) {
			motion::moveToBed({0, 0});
			enter(State::Waiting);
			return;
		}
		motion::follow(stage().pattern);
		resetFlowWatchdog();
		enter(State::Pouring);
	}

	static void finish(bool toIdle) {
		pump::off();
		finishToIdle = toIdle;
		if (motion::homed())
			motion::park();
		else
			motion::release();
		enter(State::Finishing);
	}

	bool active() { return st != State::Idle && st != State::Done && st != State::Error; }

	State state() { return st; }

	bool start(const String& recipeId, String& err) {
		if (active()) {
			err = "A brew is already running.";
			return false;
		}
		if (!recipes::load(recipeId, recipe)) {
			err = "That recipe was not found or has no stages.";
			return false;
		}
		if (!scale::connected()) {
			err = "The scale is not responding. Check the HX711 wiring.";
			return false;
		}
		if (settings.lastRecipe != recipeId) {
			settings.lastRecipe = recipeId;
			saveSettings();
		}
		error = "";
		message = "";
		stageIdx = 0;
		stageTarget = 0;
		elapsedMs = 0;
		prepMoved = false;
		scale::tare();
		if (!motion::homed()) motion::home();
		enter(State::Preparing);
		return true;
	}

	void calibratePump(String& err) {
		if (active()) {
			err = "Wait for the brew to finish first.";
			return;
		}
		error = "";
		message = "";
		prepMoved = false;
		calStep = CalStep::Prepare;
		scale::tare();
		if (!motion::homed()) motion::home();
		enter(State::PumpCal);
	}

	void pause() {
		if (st != State::Pouring && st != State::Waiting && st != State::Preparing) return;
		pump::off();
		motion::hold();
		pausedFrom = st;
		pausedInStateMs = millis() - stateStartMs;
		st = State::Paused;
	}

	void resume() {
		if (st != State::Paused) return;
		st = pausedFrom;
		stateStartMs = millis() - pausedInStateMs;
		if (st == State::Pouring) {
			motion::follow(stage().pattern);
			resetFlowWatchdog();
		} else if (st == State::Waiting) {
			motion::moveToBed({0, 0});
		} else {
			prepMoved = false;
		}
	}

	void stop() {
		if (st == State::Done || st == State::Error) {
			pump::off();
			error = "";
			enter(State::Idle);
			return;
		}
		if (active()) finish(true);
	}

	// Homed, scale tared, nozzle sitting at `where`: ready to pour.
	static bool prepared(const pour::BedPoint& where) {
		if (motion::homingFailed()) {
			fail("Homing failed: an endstop never triggered. Check the switches and wiring.");
			return false;
		}
		if (scale::busy() || !motion::homed()) return false;
		if (!prepMoved) {
			motion::moveToBed(where);
			prepMoved = true;
			return false;
		}
		return !motion::busy();
	}

	static void updatePouring(uint32_t now, uint32_t dt) {
		elapsedMs += dt;
		const float w = scale::grams();
		const float f = scale::flowGps();

		if (w > recipe.totalWater() + kOverflowMarginG) {
			fail("The scale reads more water than the recipe holds. Stopped to prevent an overflow.");
			return;
		}
		if (pour::shouldStopPour(w, f, settings.pumpLagS, stageTarget)) {
			pump::off();
			motion::moveToBed({0, 0});
			enter(State::Waiting);
			return;
		}
		const bool flowSettled = scale::flowValid() && now - stateStartMs > kFlowSettleMs;
		pump::set(fc.update(stage().flowGps, f, flowSettled, dt / 1000.0f));

		if (now - flowCheckMs >= kNoFlowMs) {
			if (w - flowCheckG < kNoFlowMinG) {
				fail("No water reached the scale. Refill the reservoir or check the pump tubing.");
				return;
			}
			resetFlowWatchdog();
		}
	}

	static void updatePumpCal(uint32_t now) {
		switch (calStep) {
			case CalStep::Prepare:
				if (!prepared({0, 0})) return;
				pump::set(1.0f);
				calStep = CalStep::Run;
				stateStartMs = now;
				return;
			case CalStep::Run:
				if (now - stateStartMs < kPumpCalMs) return;
				pump::off();
				calStep = CalStep::Settle;
				stateStartMs = now;
				return;
			case CalStep::Settle: {
															if (now - stateStartMs < kPumpCalSettleMs) return;
															const float gps = scale::grams() / (kPumpCalMs / 1000.0f);
															if (gps < 0.2f) {
																fail("Pump calibration measured almost no water. Is the reservoir primed?");
																return;
															}
															settings.pumpGpsAtFull = gps;
															saveSettings();
															message = "Pump calibrated at " + String(gps, 2) + " g/s.";
															finish(true);
															return;
														}
		}
	}

	void update() {
		const uint32_t now = millis();
		const uint32_t dt = now - lastUpdateMs;
		lastUpdateMs = now;

		switch (st) {
			case State::Preparing:
				// Travel to where the first pattern starts before any water flows.
				if (prepared(pour::patternAt(recipe.stages[0].pattern, 0))) beginStage(0);
				break;
			case State::Pouring:
				updatePouring(now, dt);
				break;
			case State::Waiting:
				elapsedMs += dt;
				if (now - stateStartMs >= (uint32_t)(stage().waitS * 1000.0f)) {
					if (stageIdx + 1 < recipe.stages.size())
						beginStage(stageIdx + 1);
					else
						finish(false);
				}
				break;
			case State::Finishing:
				if (!motion::busy()) {
					motion::release();
					enter(finishToIdle ? State::Idle : State::Done);
				}
				break;
			case State::PumpCal:
				updatePumpCal(now);
				break;
			default:
				break;
		}
	}

	static const char* stateName(State s) {
		switch (s) {
			case State::Preparing: return "preparing";
			case State::Pouring: return "pouring";
			case State::Waiting: return "waiting";
			case State::Paused: return "paused";
			case State::Finishing: return "finishing";
			case State::Done: return "done";
			case State::Error: return "error";
			case State::PumpCal: return "calibrating";
			default: return "idle";
		}
	}

	void status(JsonObject o) {
		const uint32_t now = millis();
		o["t"] = "status";
		o["state"] = stateName(st);
		if (st == State::Paused) o["pausedFrom"] = stateName(pausedFrom);
		const bool brewing = st != State::Idle && st != State::PumpCal && !recipe.stages.empty();
		if (brewing) {
			o["recipe"] = recipe.id;
			o["recipeName"] = recipe.name;
			o["stage"] = stageIdx;
			o["stages"] = recipe.stages.size();
			o["stageName"] = stage().name;
			o["target"] = stageTarget;
			o["total"] = recipe.totalWater();
			o["targetFlow"] = stage().flowGps;
			o["minTemp"] = recipe.minTempC;
			o["elapsed"] = elapsedMs / 1000.0f;
			if (st == State::Waiting || (st == State::Paused && pausedFrom == State::Waiting)) {
				const uint32_t inState = st == State::Paused ? pausedInStateMs : now - stateStartMs;
				o["waitLeft"] = fmaxf(0, stage().waitS - inState / 1000.0f);
			}
		}
		o["weight"] = scale::grams();
		o["flow"] = scale::flowGps();
		o["scaleOk"] = scale::connected();
		o["duty"] = pump::duty();
		if (thermo::present()) o["temp"] = thermo::celsius();
		o["motion"] = motion::modeName();
		o["homed"] = motion::homed();
		if (error.length()) o["error"] = error;
		if (message.length()) o["message"] = message;
	}

}  // namespace brew
