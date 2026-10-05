// OpenPour: open the file in OpenSCAD for the assembly preview, or export one
// print-ready part at a time:
//   openscad -D 'part="head"' -o stl/head.stl openpour.scad
// (`make` in this folder exports all of them.)

include <config.scad>
use <vitamins.scad>
include <parts.scad>

/* [View] */
part = "assembly";  // [assembly, base_tub, base_lid, head, arm_root, arm_tip, nozzle_holder, switch_bracket, pump_bracket, meter_clip]
arm_theta = 0;      // [-78:1:25]
carriage_r = 110;   // [66:1:175]
show_cup = true;
component = "all";  // one piece of the assembly, for hardware/render/export.sh

echo(str("Cut the 2020 column to ", column_len, " mm. Nozzle tip at ", nozzle_tip_z, " mm above the table."));

if (part == "assembly") assembly();
else print_part(part);

module print_part(name) {
  if (name == "base_tub") base_tub();
  if (name == "base_lid") translate([0, 0, -tub_height]) base_lid();
  if (name == "head") translate([0, 0, -head_bottom_z]) head();
  if (name == "arm_root") arm_root();
  if (name == "arm_tip") translate([0, 0, 6]) rotate([180, 0, 0]) arm_tip();
  if (name == "nozzle_holder")
    translate([0, 0, carriage_h + holder_t]) nozzle_holder();
  if (name == "switch_bracket") switch_bracket();
  if (name == "pump_bracket") pump_bracket();
  if (name == "meter_clip") rotate([90, 0, 0]) meter_clip();  // plate on the bed
}

// Renders its children only when `component` selects them (or is "all"), so
// hardware/render/export.sh can export the assembly one piece at a time.
module component(name) {
  if (component == "all" || component == name) children();
}

module arm_assembly() {
  component("arm") color("#e2e6e4") arm_root();
  component("arm") color("#e2e6e4") arm_tip();
  component("rail") translate([0, 0, -rail_h]) translate([rail_start, 0, 0]) mgn12_rail(rail_len);
  // face down in its pocket, shaft through the beam
  component("motor_radial")
    translate([radial_motor_x, belt_y, beam_t - radial_motor_pocket]) mirror([0, 0, 1]) nema17(radial_motor_len, 20);
  component("pulleys") {
    translate([radial_motor_x, belt_y, pulley_z[0]]) gt2_pulley();
    translate([rail_start + rail_len - 12, belt_y, pulley_z[0]]) gt2_pulley();
  }
  translate([carriage_r, 0, 0]) {
    component("carriage") translate([0, 0, -carriage_h]) mgn12h_carriage();
    component("nozzle_holder") color("#2e7da6") nozzle_holder();
    component("nozzle") translate([0, 0, -carriage_h - holder_t]) elbow_barb(nozzle_drop + 18);
  }
  component("switch") color("#444") translate([radial_switch_screws[0] - 6, -14, -12]) cube([20, 6.4, 10]);
}

module assembly() {
  component("base_tub") color("#e2e6e4") base_tub();
  component("base_lid") color("#cfd6d3") base_lid();
  component("coaster") translate([dripper_offset, 0, lid_top]) coaster(coaster_d, coaster_t);

  component("column") translate([column_x, 0, column_bottom_z]) ext2020(column_len);
  // flow meter on the column's side face, between the pump and the nozzle
  meter_z = head_bottom_z - 110;  // well below the arm and its pulleys
  translate([column_x, -ext / 2, meter_z]) rotate([0, 0, 180]) translate([-(meter_body[1] + 2 * wall + 20) / 2, 0, 0]) {
    component("meter_clip") color("#2e7da6") meter_clip();
    component("flow_meter") translate([10 + wall + meter_body[1] / 2, 4 + meter_body[2] / 2, 20 - meter_body[0] / 2])
      rotate([0, 0, 90]) rotate([0, -90, 0]) flow_meter(meter_body, meter_barb_len);
  }
  component("head") color("#e2e6e4") head();
  component("motor_theta") translate([0, 0, head_top_z]) mirror([0, 0, 1]) nema17();  // face down on the head
  component("bearings") for (z = [head_top_z - coupler_len - collar_t - 3 - bearing_t, head_bottom_z])
    translate([0, 0, z]) bearing608();
  component("shaft") color("silver") translate([0, 0, beam_bottom_z])
    cylinder(d = shaft_d, h = head_top_z - coupler_len / 2 - beam_bottom_z);

  translate([0, 0, beam_bottom_z]) rotate(arm_theta) arm_assembly();

  if (show_cup) translate([dripper_offset, 0, cup_base_z]) {
    component("mug") mug(cup_height);
    component("dripper") translate([0, 0, cup_height]) dripper(dripper_height);
  }
}
