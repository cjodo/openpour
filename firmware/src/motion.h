#pragma once
#include <pattern.h>

// Two-axis polar arm: theta (direct-drive arm swing) and radial (belt-driven
// carriage along the arm). Both axes run in velocity mode so the nozzle can
// follow continuously changing patterns, including direction reversals.
namespace motion {

void begin();
void update();

void home();
bool homed();
bool homingFailed();
bool busy();  // homing, or still travelling to a fixed target

void moveTo(const pour::ArmPose& target);
void moveToBed(const pour::BedPoint& p);  // relative to the dripper centre
void park();
void follow(const pour::PatternParams& p);  // starts at t = 0 now
void hold();                                // stay where we are
void jog(float dr, float dThetaDeg);
void setCenterHere();  // store current pose as the dripper centre
void release();        // de-energise motors (position is lost)

pour::ArmPose pose();
const char* modeName();

}  // namespace motion
