#include "motion.h"

#include <Arduino.h>
#include <FastAccelStepper.h>

#include "pins.h"
#include "settings.h"

using pour::ArmPose;
using pour::BedPoint;
using pour::PatternParams;

namespace motion {

static constexpr float kThetaMaxDegS = 120.0f;
static constexpr float kThetaAccelDegS2 = 900.0f;
static constexpr float kRadialMaxMmS = 150.0f;
static constexpr float kRadialAccelMmS2 = 1500.0f;
static constexpr float kHomeDegS = 20.0f;
static constexpr float kHomeMmS = 15.0f;
static constexpr uint32_t kHomeTimeoutMs = 20000;
static constexpr uint32_t kControlPeriodMs = 10;
static constexpr float kKp = 15.0f;      // 1/s, position error -> velocity
static constexpr float kMinHz = 20.0f;   // below this, just stop
static constexpr float kDeadband = 2.0f; // steps

enum class Mode { Released, Homing, Hold, Track };
enum class HomeStep { Radial, Theta };

static FastAccelStepperEngine engine;
static FastAccelStepper* thetaM = nullptr;
static FastAccelStepper* radialM = nullptr;

static Mode mode = Mode::Released;
static HomeStep homeStep = HomeStep::Radial;
static uint32_t homeStartMs = 0;
static bool isHomed = false;
static bool homeFailed = false;

static ArmPose holdTarget{0, 0};
static PatternParams track;
static uint32_t trackStartMs = 0;
static uint32_t lastControlMs = 0;

static bool endstop(uint8_t pin) { return digitalRead(pin) == LOW; }

static ArmPose centre() { return {settings.centerR, settings.centerThetaDeg}; }

static ArmPose clampPose(ArmPose p) {
  p.r = constrain(p.r, settings.radialMinMm, settings.radialMaxMm);
  p.thetaDeg = constrain(p.thetaDeg, settings.thetaMinDeg, settings.thetaMaxDeg);
  return p;
}

static void configure(FastAccelStepper* m, uint8_t dirPin, bool invert, float accelStepsS2) {
  m->setDirectionPin(dirPin, !invert);
  m->setEnablePin(PIN_STEPPER_EN, true);
  m->setAutoEnable(false);
  m->setAcceleration((int32_t)accelStepsS2);
}

void begin() {
  pinMode(PIN_THETA_ENDSTOP, INPUT_PULLUP);
  pinMode(PIN_RADIAL_ENDSTOP, INPUT_PULLUP);
  engine.init();
  thetaM = engine.stepperConnectToPin(PIN_THETA_STEP);
  radialM = engine.stepperConnectToPin(PIN_RADIAL_STEP);
  configure(thetaM, PIN_THETA_DIR, settings.invertTheta,
            kThetaAccelDegS2 * settings.thetaStepsPerDeg);
  configure(radialM, PIN_RADIAL_DIR, settings.invertRadial,
            kRadialAccelMmS2 * settings.radialStepsPerMm);
  thetaM->disableOutputs();
  radialM->disableOutputs();
}

ArmPose pose() {
  return {radialM->getCurrentPosition() / settings.radialStepsPerMm,
          thetaM->getCurrentPosition() / settings.thetaStepsPerDeg};
}

// Velocity-mode tracking: feed-forward from the path plus a correction that
// is proportional for small errors and time-optimal (sqrt) for large ones.
static void driveAxis(FastAccelStepper* m, float targetSteps, float ffHz, float vmaxHz,
                      float accel) {
  const float err = targetSteps - (float)m->getCurrentPosition();
  if (fabsf(err) <= kDeadband && fabsf(ffHz) < kMinHz) {
    if (m->isRunning()) m->stopMove();
    return;
  }
  const float corr = copysignf(fminf(sqrtf(accel * fabsf(err)), kKp * fabsf(err)), err);
  const float v = constrain(ffHz + corr, -vmaxHz, vmaxHz);
  if (fabsf(v) < kMinHz) {
    if (m->isRunning()) m->stopMove();
    return;
  }
  m->setSpeedInHz((uint32_t)fabsf(v));
  if (v > 0)
    m->runForward();
  else
    m->runBackward();
}

static void control() {
  ArmPose tgt = holdTarget;
  float ffR = 0, ffT = 0;
  if (mode == Mode::Track) {
    const float dt = kControlPeriodMs / 1000.0f;
    const float t = (millis() - trackStartMs) / 1000.0f;
    tgt = clampPose(pour::bedToArm(pour::patternAt(track, t), centre()));
    const ArmPose next = clampPose(pour::bedToArm(pour::patternAt(track, t + dt), centre()));
    ffR = (next.r - tgt.r) / dt * settings.radialStepsPerMm;
    ffT = (next.thetaDeg - tgt.thetaDeg) / dt * settings.thetaStepsPerDeg;
  }
  driveAxis(radialM, tgt.r * settings.radialStepsPerMm, ffR,
            kRadialMaxMmS * settings.radialStepsPerMm, kRadialAccelMmS2 * settings.radialStepsPerMm);
  driveAxis(thetaM, tgt.thetaDeg * settings.thetaStepsPerDeg, ffT,
            kThetaMaxDegS * settings.thetaStepsPerDeg, kThetaAccelDegS2 * settings.thetaStepsPerDeg);
}

static void startHomingAxis(FastAccelStepper* m, float speedHz) {
  m->setSpeedInHz((uint32_t)speedHz);
  m->runBackward();
}

static void homingStep() {
  if (millis() - homeStartMs > kHomeTimeoutMs) {
    thetaM->forceStopAndNewPosition(0);
    radialM->forceStopAndNewPosition(0);
    homeFailed = true;
    release();
    return;
  }
  if (homeStep == HomeStep::Radial) {
    if (endstop(PIN_RADIAL_ENDSTOP)) {
      radialM->forceStopAndNewPosition(
          (int32_t)lroundf(settings.radialHomeMm * settings.radialStepsPerMm));
      homeStep = HomeStep::Theta;
      startHomingAxis(thetaM, kHomeDegS * settings.thetaStepsPerDeg);
    }
  } else if (endstop(PIN_THETA_ENDSTOP)) {
    thetaM->forceStopAndNewPosition(
        (int32_t)lroundf(settings.thetaHomeDeg * settings.thetaStepsPerDeg));
    isHomed = true;
    park();
  }
}

void update() {
  if (mode == Mode::Homing) {
    homingStep();  // polled every loop for a tight stop on the switch
    return;
  }
  if (mode != Mode::Hold && mode != Mode::Track) return;
  const uint32_t now = millis();
  if (now - lastControlMs < kControlPeriodMs) return;
  lastControlMs = now;
  control();
}

void home() {
  isHomed = false;
  homeFailed = false;
  thetaM->enableOutputs();
  radialM->enableOutputs();
  mode = Mode::Homing;
  homeStep = HomeStep::Radial;
  homeStartMs = millis();
  startHomingAxis(radialM, kHomeMmS * settings.radialStepsPerMm);
}

bool homed() { return isHomed; }
bool homingFailed() { return homeFailed; }

bool busy() {
  if (mode == Mode::Homing) return true;
  if (mode != Mode::Hold) return false;
  const float er = fabsf(holdTarget.r * settings.radialStepsPerMm - radialM->getCurrentPosition());
  const float et = fabsf(holdTarget.thetaDeg * settings.thetaStepsPerDeg - thetaM->getCurrentPosition());
  return er > kDeadband * 2 || et > kDeadband * 2 || radialM->isRunning() || thetaM->isRunning();
}

void moveTo(const ArmPose& target) {
  if (!isHomed) return;
  holdTarget = clampPose(target);
  mode = Mode::Hold;
}

void moveToBed(const BedPoint& p) { moveTo(pour::bedToArm(p, centre())); }

void park() { moveTo({settings.parkR, settings.parkThetaDeg}); }

void follow(const PatternParams& p) {
  if (!isHomed) return;
  track = p;
  trackStartMs = millis();
  mode = Mode::Track;
}

void hold() {
  if (!isHomed) return;
  holdTarget = clampPose(pose());
  mode = Mode::Hold;
}

void jog(float dr, float dThetaDeg) {
  if (!isHomed) return;
  const ArmPose base = mode == Mode::Hold ? holdTarget : pose();
  moveTo({base.r + dr, base.thetaDeg + dThetaDeg});
}

void setCenterHere() {
  const ArmPose p = mode == Mode::Hold ? holdTarget : pose();
  settings.centerR = p.r;
  settings.centerThetaDeg = p.thetaDeg;
  saveSettings();
}

void release() {
  thetaM->stopMove();
  radialM->stopMove();
  thetaM->disableOutputs();
  radialM->disableOutputs();
  isHomed = false;
  mode = Mode::Released;
}

const char* modeName() {
  switch (mode) {
    case Mode::Homing: return "homing";
    case Mode::Hold: return busy() ? "moving" : "holding";
    case Mode::Track: return "pouring";
    default: return "released";
  }
}

}  // namespace motion
