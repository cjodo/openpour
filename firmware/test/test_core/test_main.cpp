#include <unity.h>

#include "flow.h"
#include "kinematics.h"
#include "pattern.h"

using namespace pour;

static const ArmPose kCentre{110.0f, 5.0f};

void setUp() {}
void tearDown() {}

void test_centre_maps_to_centre_pose() {
  ArmPose a = bedToArm({0, 0}, kCentre);
  TEST_ASSERT_FLOAT_WITHIN(1e-4, kCentre.r, a.r);
  TEST_ASSERT_FLOAT_WITHIN(1e-4, kCentre.thetaDeg, a.thetaDeg);
}

void test_round_trip() {
  for (float rho = 0; rho <= 40; rho += 5) {
    for (float phi = -3.0f; phi <= 3.0f; phi += 0.5f) {
      BedPoint b = armToBed(bedToArm({rho, phi}, kCentre), kCentre);
      TEST_ASSERT_FLOAT_WITHIN(1e-3, rho, b.rho);
      if (rho > 0) TEST_ASSERT_FLOAT_WITHIN(1e-3, phi, b.phi);
    }
  }
}

void test_radial_and_tangential_points() {
  // Straight out along the arm: only r changes.
  ArmPose out = bedToArm({20, 0}, kCentre);
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 130.0f, out.r);
  TEST_ASSERT_FLOAT_WITHIN(1e-4, kCentre.thetaDeg, out.thetaDeg);
  // Sideways: r grows a little, theta swings by atan(20/110).
  ArmPose side = bedToArm({20, kPi / 2}, kCentre);
  TEST_ASSERT_FLOAT_WITHIN(1e-3, std::sqrt(110.0f * 110.0f + 400.0f), side.r);
  TEST_ASSERT_FLOAT_WITHIN(1e-3, kCentre.thetaDeg + rad2deg(std::atan2(20.0f, 110.0f)), side.thetaDeg);
}

void test_circle_keeps_radius() {
  PatternParams p{Pattern::Circle, 25, 1.0f};
  for (float t = 0; t < 3; t += 0.1f) TEST_ASSERT_FLOAT_WITHIN(1e-4, 25.0f, patternAt(p, t).rho);
}

void test_spiral_sweeps_out_and_back() {
  PatternParams p{Pattern::Spiral, 30, 1.0f};
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 0.0f, patternAt(p, 0).rho);
  TEST_ASSERT_FLOAT_WITHIN(1e-3, 30.0f, patternAt(p, kSpiralTurns).rho);
  TEST_ASSERT_FLOAT_WITHIN(1e-3, 0.0f, patternAt(p, 2 * kSpiralTurns).rho);
  for (float t = 0; t < 20; t += 0.05f) {
    float rho = patternAt(p, t).rho;
    TEST_ASSERT_TRUE(rho >= -1e-4 && rho <= 30.0f + 1e-4);
  }
}

void test_center_pattern_is_still() {
  PatternParams p{Pattern::Center, 30, 1.0f};
  TEST_ASSERT_FLOAT_WITHIN(1e-6, 0.0f, patternAt(p, 7.3f).rho);
}

void test_parse_pattern() {
  TEST_ASSERT_TRUE(parsePattern("spiral") == Pattern::Spiral);
  TEST_ASSERT_TRUE(parsePattern("circle") == Pattern::Circle);
  TEST_ASSERT_TRUE(parsePattern("nonsense") == Pattern::Center);
  TEST_ASSERT_TRUE(parsePattern(nullptr) == Pattern::Center);
}

void test_flow_feed_forward_and_clamp() {
  FlowController fc;
  fc.gpsAtFull = 8.0f;
  fc.minDuty = 0.2f;
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 0.5f, fc.update(4.0f, 0, false, 0.02f));
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 0.2f, fc.update(0.5f, 0, false, 0.02f));  // min duty
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 1.0f, fc.update(20.0f, 0, false, 0.02f)); // max duty
  TEST_ASSERT_FLOAT_WITHIN(1e-4, 0.0f, fc.update(0.0f, 0, false, 0.02f));  // off
}

void test_flow_integral_raises_duty_when_slow() {
  FlowController fc;
  fc.gpsAtFull = 8.0f;
  float first = fc.update(4.0f, 2.0f, true, 0.1f);
  float later = first;
  for (int i = 0; i < 50; i++) later = fc.update(4.0f, 2.0f, true, 0.1f);
  TEST_ASSERT_TRUE(later > first);
}

void test_should_stop_pour_accounts_for_lag() {
  TEST_ASSERT_FALSE(shouldStopPour(40, 4, 0.5f, 50));
  TEST_ASSERT_TRUE(shouldStopPour(48.5f, 4, 0.5f, 50));
  TEST_ASSERT_FALSE(shouldStopPour(49, -10, 0.5f, 50));  // negative flow ignored
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_centre_maps_to_centre_pose);
  RUN_TEST(test_round_trip);
  RUN_TEST(test_radial_and_tangential_points);
  RUN_TEST(test_circle_keeps_radius);
  RUN_TEST(test_spiral_sweeps_out_and_back);
  RUN_TEST(test_center_pattern_is_still);
  RUN_TEST(test_parse_pattern);
  RUN_TEST(test_flow_feed_forward_and_clamp);
  RUN_TEST(test_flow_integral_raises_duty_when_slow);
  RUN_TEST(test_should_stop_pour_accounts_for_lag);
  return UNITY_END();
}
