// Printed parts. Modules are drawn in assembly position; openpour.scad
// re-orients each one for printing.
//
// Arm parts use arm-local coordinates: origin on the pivot axis at the
// underside of the beam (where the rail mounts), +X along the arm.

include <config.scad>

e = 0.01;
C = clearance;

module rounded_box(size, r) {
  hull() for (x = [r, size[0] - r], y = [r, size[1] - r])
    translate([x, y, 0]) cylinder(r = r, h = size[2]);
}

module counterbored(d, head_d, head_h, len) {
  // hole along +Z from z=0, head recess at the bottom
  translate([0, 0, -e]) cylinder(d = d, h = len + 2 * e);
  translate([0, 0, -e]) cylinder(d = head_d, h = head_h + e);
}

// ------------------------------------------------------------------ base

base_len = base_front - base_back;
corner_inset = 6.5;
function corner_positions() = [
  for (x = [base_back + corner_inset, base_front - corner_inset],
       y = [-base_width / 2 + corner_inset, base_width / 2 - corner_inset]) [x, y]
];

// 50 x 70 mm double-sided prototype board for the ESP32 (M2 holes).
proto_board_origin = [-17, -35];
proto_board_holes = [[2, 2], [46 + 2, 2], [2, 66 + 2], [46 + 2, 66 + 2]];
button_pos = [base_front - 18, -base_width / 2 + 18];

module base_tub() {
  sock = ext + 10;
  difference() {
    union() {
      difference() {
        translate([base_back, -base_width / 2, 0]) rounded_box([base_len, base_width, tub_height], 8);
        translate([base_back + wall, -base_width / 2 + wall, floor_t])
          rounded_box([base_len - 2 * wall, base_width - 2 * wall, tub_height], 8 - wall);
      }
      for (p = corner_positions()) translate(p) cylinder(d = corner_boss_d, h = tub_height);
      // column socket
      translate([column_x - sock / 2, -sock / 2, 0]) cube([sock, sock, tub_height]);
      // proto board standoffs
      for (h = proto_board_holes) translate(proto_board_origin + h) cylinder(d = 6, h = floor_t + 5);
    }
    for (p = corner_positions()) translate([p[0], p[1], floor_t]) cylinder(d = screw_m3_tap, h = tub_height);
    // column pocket + M5 bolt up into the tapped extrusion end
    translate([column_x - ext / 2 - C, -ext / 2 - C, column_bottom_z]) cube([ext + 2 * C, ext + 2 * C, tub_height]);
    translate([column_x, 0, 0]) counterbored(screw_m5_clear, 10, 4, column_bottom_z);
    for (h = proto_board_holes) translate([proto_board_origin[0] + h[0], proto_board_origin[1] + h[1], 1]) cylinder(d = 1.7, h = 10);
    // rear wall: DC barrel jack (panel mount) and ESP32 USB access
    translate([base_back - 1, 40, 20]) rotate([0, 90, 0]) cylinder(d = 11.5 + C, h = wall + 2);
    translate([base_back - 1, -12, floor_t + 6]) cube([wall + 2, 14, 9]);
    // rubber foot recesses
    for (p = corner_positions()) translate([p[0], p[1], -e]) cylinder(d = 10.5, h = 1);
  }
}

module base_lid() {
  difference() {
    union() {
      translate([base_back, -base_width / 2, tub_height]) rounded_box([base_len, base_width, lid_t], 8);
      // coaster locating ring, centred under the dripper
      translate([dripper_offset, 0, lid_top - e]) difference() {
        cylinder(d = coaster_d + 2 * C + 2 * wall, h = coaster_ring_h + e);
        translate([0, 0, -1]) cylinder(d = coaster_d + 2 * C, h = coaster_ring_h + 2);
      }
    }
    for (p = corner_positions()) translate([p[0], p[1], tub_height - e]) {
      cylinder(d = screw_m3_clear, h = lid_t + 1);
      translate([0, 0, lid_t - 1.8]) cylinder(d1 = screw_m3_clear, d2 = 6.5, h = 1.8 + e);
    }
    translate([column_x - ext / 2 - C, -ext / 2 - C, tub_height - 1]) cube([ext + 2 * C, ext + 2 * C, lid_t + 2]);
    // momentary button (12 mm)
    translate([button_pos[0], button_pos[1], tub_height - 1]) cylinder(d = 12 + C, h = lid_t + 2);
    // cable pass-through beside the column: motors, endstops, pump, probe, flow meter
    translate([column_x - 8, ext / 2 + 5, tub_height - 1]) rounded_box([16, 10, lid_t + 2], 2);
    // vents over the stepper drivers
    for (i = [0 : 5]) translate([-5 + i * 7, -base_width / 2 + 12, tub_height - 1]) rounded_box([3, 30, lid_t + 2], 1.4);
  }
}

// ------------------------------------------------------------------ head

head_r = bearing_od / 2 + 6;

module head() {
  hb = head_bottom_z;
  ht = head_top_z;
  blk = ext + 16;
  cavity = coupler_len + collar_t + 3;
  difference() {
    union() {
      translate([column_x - blk / 2, -blk / 2, hb]) cube([blk, blk, head_h]);
      hull() {
        translate([column_x, -14, hb]) cube([e, 28, head_h]);
        translate([0, 0, hb]) cylinder(r = head_r, h = head_h);
      }
      translate([-nema / 2, -nema / 2, ht - 6]) cube([nema, nema, 6]);
    }
    // column bore and clamp screws into T-nuts on both side slots
    translate([column_x - ext / 2 - C, -ext / 2 - C, hb - 1]) cube([ext + 2 * C, ext + 2 * C, head_h + 2]);
    for (z = [hb + 14, ht - 14], s = [-1, 1])
      translate([column_x, s * (ext / 2 + 8 + 1), z]) rotate([90, 0, 0]) {
        cylinder(d = screw_m5_clear, h = 20, center = true);
        translate([0, 0, s > 0 ? -12 : 4]) cylinder(d = 10, h = 8);
      }
    // motor: pilot, coupler + collar cavity, 4 screws from below
    translate([0, 0, ht - cavity]) cylinder(d = coupler_d + 4, h = cavity + 1);
    for (x = [-1, 1], y = [-1, 1]) translate([x * nema_hole_spacing / 2, y * nema_hole_spacing / 2, ht - 7])
      cylinder(d = screw_m3_clear, h = 8);
    // bearings: top one under the cavity, bottom one from below
    translate([0, 0, ht - cavity - bearing_t]) cylinder(d = bearing_od + C, h = bearing_t + e);
    translate([0, 0, hb - e]) cylinder(d = bearing_od + C, h = bearing_t + e);
    translate([0, 0, hb]) cylinder(d = 16, h = head_h);
    // theta switch mount (two M3 pilots on the underside)
    for (p = theta_switch_mount_holes()) translate([p[0], p[1], hb - e]) cylinder(d = screw_m3_tap, h = 10);
  }
}

// The finger on the hub sweeps a circle of radius finger_r. The switch sits on
// that circle where the finger arrives at theta = thetaHomeDeg.
theta_home = -75;
function theta_switch_angle() = 180 + theta_home - 8;
function theta_switch_mount_holes() = [
  for (a = [theta_switch_angle() - 14, theta_switch_angle() + 2]) [ (head_r - 3) * cos(a), (head_r - 3) * sin(a) ]
];

// ------------------------------------------------------------------ arm

// The belt clamp must stay clear of the radial pulley, so the carriage homes
// at r ~ 66 mm (firmware radialHomeMm) against a switch under the beam.
radial_switch_screws = [28, 38];

function rail_holes() = [for (x = [rail_hole_first : rail_hole_pitch : rail_len]) rail_start + x];

module arm_root() {
  beam_y_len = beam_y[1] - beam_y[0];
  difference() {
    union() {
      translate([beam_back, beam_y[0], 0]) rounded_box([beam_front - beam_back, beam_y_len, beam_t], 4);
      cylinder(d = hub_d, h = hub_h);
      // endstop finger, pointing back at the column when theta = 0
      hull() {
        translate([-hub_d / 2 + 2, -3, beam_t]) cube([1, 6, hub_h - beam_t - 4]);
        translate([-finger_r, -2, beam_t]) cube([2, 4, hub_h - beam_t - 4]);
      }
      // stiffening rib along the rail line
      translate([hub_d / 2 - 2, -4, beam_t - e]) cube([beam_front - hub_d / 2 - 2 - 6, 6, 6]);
    }
    // shaft bore + clamp slit + clamp screw
    translate([0, 0, -1]) cylinder(d = shaft_d + C, h = hub_h + 2);
    translate([0, -0.75, beam_t]) cube([hub_d, 1.5, hub_h]);
    translate([hub_d / 2 - 4, 0, beam_t + (hub_h - beam_t) / 2]) rotate([90, 0, 0]) {
      cylinder(d = screw_m3_clear, h = hub_d, center = true);
      translate([0, 0, 5]) cylinder(d = 6.4, h = 10, $fn = 6);
    }
    // rail screws from below into nuts on top
    for (x = rail_holes()) if (x < beam_front - 4) translate([x, 0, -e]) {
      cylinder(d = screw_m3_clear, h = beam_t + 10);
      translate([0, 0, beam_t - 2.5]) cylinder(d = 6.4, h = 10, $fn = 6);
    }
    // radial motor: pocket, shaft, pilot, screws
    translate([radial_motor_x, belt_y, 0]) {
      translate([-nema / 2 - C, -nema / 2 - C, beam_t - radial_motor_pocket]) cube([nema + 2 * C, nema + 2 * C, 20]);
      translate([0, 0, -1]) cylinder(d = nema_boss_d + 1, h = beam_t + 2);
      for (x = [-1, 1], y = [-1, 1]) translate([x * nema_hole_spacing / 2, y * nema_hole_spacing / 2, -1]) {
        cylinder(d = screw_m3_clear, h = beam_t + 2);
        translate([0, 0, -e]) cylinder(d = 6, h = 1 + 1.5);
      }
    }
    // radial endstop bracket screws
    for (x = radial_switch_screws) translate([x, -12, -e]) cylinder(d = screw_m3_tap, h = 6);
  }
}

module arm_tip() {
  x0 = rail_start + rail_len - 25;
  last_hole = rail_holes()[len(rail_holes()) - 1];
  idler_x = rail_start + rail_len - 4;
  difference() {
    union() {
      translate([x0, -10, 0]) cube([25, belt_y + 23, 6]);                       // sits on the rail's mount face
      translate([x0, belt_y + 9, pulley_z[0] - 5]) cube([25, 4, 6 - pulley_z[0] + 5]);  // side wall
      translate([x0, belt_y - 9, pulley_z[0] - 5]) cube([25, 22, 5]);           // lower fork
    }
    // rail screw (from below through the rail) into a nut
    translate([last_hole, 0, -e]) {
      cylinder(d = screw_m3_clear, h = 10);
      translate([0, 0, 6 - 2.5]) cylinder(d = 6.4, h = 5, $fn = 6);
    }
    // idler bolt slot for belt tension
    hull() for (dx = [-4, 2]) translate([idler_x - 8 + dx, belt_y, pulley_z[0] - 10]) cylinder(d = 5 + C, h = 40);
  }
}

module nozzle_holder() {
  z0 = -carriage_h - holder_t;
  difference() {
    union() {
      translate([-15, -15, z0]) rounded_box([30, 30, holder_t], 3);
      // belt clamp on the inner belt span
      translate([-12, carriage_w / 2 + 0.6, z0]) cube([24, 8, -z0 - 4]);
      // nozzle boss
      translate([0, 0, z0 - 18]) cylinder(d = nozzle_d + 8, h = 18 + e);
    }
    for (x = [-1, 1], y = [-1, 1]) translate([x * carriage_hole_spacing / 2, y * carriage_hole_spacing / 2, z0 - e]) {
      cylinder(d = screw_m3_clear, h = holder_t + 1);
      cylinder(d = 6, h = 3);
    }
    // belt slot (both belt ends tuck in from either side) + pinch screw
    translate([-13, belt_y - pulley_pitch_r - 0.8, pulley_z[0]]) cube([26, 1.6, pulley_z[1] - pulley_z[0]]);
    translate([0, carriage_w / 2, (pulley_z[0] + pulley_z[1]) / 2]) rotate([-90, 0, 0]) cylinder(d = screw_m3_tap, h = 12);
    // nozzle bore + pinch screw
    translate([0, 0, z0 - 20]) cylinder(d = nozzle_d + C, h = 40);
    translate([0, 0, z0 - 9]) rotate([90, 0, 0]) cylinder(d = screw_m3_tap, h = 20);
  }
}

// One bracket fits both lever switches: a tab with two M3 slots and a plate
// with the switch's two holes.
module switch_bracket() {
  difference() {
    union() {
      cube([22, 12, 3]);
      cube([22, 3, 12]);
    }
    for (x = [5, 15]) hull() for (dy = [-1, 1]) translate([x, 7.5 + dy, -1]) cylinder(d = screw_m3_clear, h = 5);
    for (x = [11 - switch_hole_spacing / 2, 11 + switch_hole_spacing / 2]) translate([x, -1, 8]) rotate([-90, 0, 0])
      cylinder(d = switch_hole_d, h = 5);
  }
}

// Clamps to the column with two M5 T-nuts; slots fit most small pumps.
module pump_bracket() {
  difference() {
    union() {
      cube([60, 70, 4]);
      translate([20, 0, 0]) cube([20, 4, 30]);
    }
    for (x = [8 : 11 : 52]) hull() for (y = [14, 62]) translate([x, y, -1]) cylinder(d = screw_m3_clear, h = 6);
    for (z = [10, 22]) translate([30, -1, z]) rotate([-90, 0, 0]) cylinder(d = screw_m5_clear, h = 6);
  }
}

// Holds the flow meter on the column with two M5 T-nuts, barbs vertical so
// it stays full of water; two cable ties go through the slots around the body.
module meter_clip() {
  w = meter_body[1] + 2 * wall;
  difference() {
    union() {
      cube([w + 20, 4, 40]);                       // plate on the column
      translate([10, 0, 0]) cube([w, meter_body[2] / 2 + 4, 40]);  // cradle
    }
    // body seat
    translate([10 + wall - C, 4, -1]) cube([meter_body[1] + 2 * C, meter_body[2], 42]);
    // cable-tie slots
    for (z = [8, 28]) translate([5, -1, z]) cube([w + 10, meter_body[2] + 10, 4]);
    // T-nut screws either side of the cradle
    for (x = [5, w + 15]) translate([x, -1, 20]) rotate([-90, 0, 0]) cylinder(d = screw_m5_clear, h = 6);
  }
}
