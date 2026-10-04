// Simplified stand-ins for purchased parts, for the assembly preview only.

module ext2020(len) {
  color("silver") difference() {
    translate([-10, -10, 0]) cube([20, 20, len]);
    for (a = [0 : 90 : 270]) rotate(a) translate([7, -3, -1]) cube([4, 6, len + 2]);
    translate([0, 0, -1]) cylinder(d = 4.2, h = len + 2);
  }
}

module nema17(len = 40, shaft = 24) {
  color("#333") translate([-21.15, -21.15, -len]) cube([42.3, 42.3, len]);
  color("silver") {
    cylinder(d = 22, h = 2);
    cylinder(d = 5, h = shaft);
  }
}

module bearing608() {
  color("#999") difference() {
    cylinder(d = 22, h = 7);
    translate([0, 0, -1]) cylinder(d = 8, h = 9);
  }
}

module gt2_pulley() {
  color("gold") {
    cylinder(d = 16, h = 1.5);
    cylinder(d = 12.2, h = 14);
    translate([0, 0, 7]) cylinder(d = 16, h = 1.5);
  }
}

module mgn12_rail(len) {
  color("#b0b6ba") translate([0, -6, 0]) cube([len, 12, 8]);
}

module mgn12h_carriage() {
  color("#5b6670") translate([-22.7, -13.5, 0]) cube([45.4, 27, 10]);
}

module coaster(d, t = 4) {
  color("#b98c5a") cylinder(d = d, h = t);
}

// Inline hall-effect flow meter, flow along +X.
module flow_meter(body, barb) {
  color("#3b3f45") translate([0, -body[1] / 2, -body[2] / 2]) cube(body);
  color("#c9ccd0") for (x = [-barb, body[0]]) translate([x, 0, 0]) rotate([0, 90, 0]) cylinder(d = 6, h = barb);
}

module mug(h = 110, d = 80) {
  color("white", 0.6) difference() {
    cylinder(d = d, h = h);
    translate([0, 0, 4]) cylinder(d = d - 6, h = h);
  }
}

module dripper(h = 90) {
  color("#d8e6ee", 0.7) {
    cylinder(d = 100, h = 6);
    translate([0, 0, 6]) cylinder(d1 = 40, d2 = 116, h = h - 6);
  }
}

module elbow_barb(len = 30) {
  color("silver") {
    translate([0, 0, -len]) cylinder(d = 6, h = len);
    rotate([90, 0, 0]) cylinder(d = 6, h = 22);
  }
}
