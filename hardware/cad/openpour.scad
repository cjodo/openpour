// OpenPour: open the file in OpenSCAD for the assembly preview, or export one
// print-ready part at a time:
//   openscad -D 'part="head"' -o stl/head.stl openpour.scad
// (`make` in this folder exports all of them.)

include <config.scad>
use <vitamins.scad>
include <parts.scad>

/* [View] */
part = "assembly";  // [assembly, base_tub, base_lid, platform, head, arm_root, arm_tip, nozzle_holder, switch_bracket, pump_bracket]
arm_theta = 0;      // [-78:1:25]
carriage_r = 110;   // [66:1:175]
show_cup = true;

echo(str("Cut the 2020 column to ", column_len, " mm. Nozzle tip at ", nozzle_tip_z, " mm above the table."));

if (part == "assembly") assembly();
else print_part(part);

module print_part(name) {
  if (name == "base_tub") base_tub();
  if (name == "base_lid") translate([0, 0, -tub_height]) base_lid();
  if (name == "platform")  // upside down: the coaster face prints on the bed
    translate([0, 0, platform_top]) rotate([180, 0, 0]) platform();
  if (name == "head") translate([0, 0, -head_bottom_z]) head();
  if (name == "arm_root") arm_root();
  if (name == "arm_tip") translate([0, 0, 6]) rotate([180, 0, 0]) arm_tip();
  if (name == "nozzle_holder")
    translate([0, 0, carriage_h + holder_t]) nozzle_holder();
  if (name == "switch_bracket") switch_bracket();
  if (name == "pump_bracket") pump_bracket();
}

module arm_assembly() {
  color("#e2e6e4") arm_root();
  color("#e2e6e4") arm_tip();
  translate([0, 0, -rail_h]) translate([rail_start, 0, 0]) mgn12_rail(rail_len);
  // face down in its pocket, shaft through the beam
  translate([radial_motor_x, belt_y, beam_t - radial_motor_pocket]) mirror([0, 0, 1]) nema17(radial_motor_len, 20);
  translate([radial_motor_x, belt_y, pulley_z[0]]) gt2_pulley();
  translate([rail_start + rail_len - 12, belt_y, pulley_z[0]]) gt2_pulley();
  translate([carriage_r, 0, 0]) {
    translate([0, 0, -carriage_h]) mgn12h_carriage();
    color("#2e7da6") nozzle_holder();
    translate([0, 0, -carriage_h - holder_t]) elbow_barb(nozzle_drop + 18);
  }
  color("#444") translate([radial_switch_screws[0] - 6, -14, -12]) cube([20, 6.4, 10]);
}

module assembly() {
  color("#e2e6e4") base_tub();
  color("#cfd6d3") base_lid();
  color("#2e7da6") platform();
  translate([dripper_offset, 0, platform_top - coaster_recess]) coaster(coaster_d);
  translate([lc_x0, -lc_w / 2, lc_z0]) load_cell(lc_len, lc_w, lc_h);

  translate([column_x, 0, column_bottom_z]) ext2020(column_len);
  color("#e2e6e4") head();
  translate([0, 0, head_top_z]) mirror([0, 0, 1]) nema17();  // face down on the head
  for (z = [head_top_z - coupler_len - collar_t - 3 - bearing_t, head_bottom_z])
    translate([0, 0, z]) bearing608();
  color("silver") translate([0, 0, beam_bottom_z]) cylinder(d = shaft_d, h = head_top_z - coupler_len / 2 - beam_bottom_z);

  translate([0, 0, beam_bottom_z]) rotate(arm_theta) arm_assembly();

  if (show_cup) translate([dripper_offset, 0, platform_top]) {
    mug(cup_height);
    translate([0, 0, cup_height]) dripper(dripper_height);
  }
}
