// OpenPour: every dimension you might want to change lives here.
// Units: mm. Origin: the arm pivot axis at table level. +X points from the
// pivot to the dripper (towards the user), +Z is up.

/* [Your cups and dripper] */
cup_height = 110;        // tallest mug or server you'll brew into
dripper_height = 90;     // dripper body including its base
nozzle_clearance = 20;   // gap between dripper rim and nozzle tip

/* [Layout] */
dripper_offset = 110;    // pivot axis to dripper centre (firmware: centerR)
column_x = -35;          // 2020 column centre
base_back = -52;         // base rear edge
base_front = 163;        // base front edge
base_width = 150;

/* [Print settings] */
wall = 2.4;
clearance = 0.25;        // added to holes and sockets; raise for loose printers
$fn = 48;

/* [Base] */
floor_t = 3;
tub_height = 43;         // floor bottom to lid underside
lid_t = 3;
corner_boss_d = 9;
screw_m3_tap = 2.6;      // self-tapping into plastic; 4.0 for heat-set inserts
screw_m3_clear = 3.4;
screw_m4_clear = 4.4;
screw_m5_clear = 5.4;

/* [Cup rest] */
coaster_d = 95;              // cork/silicone coaster: printed parts must not touch hot cups
coaster_t = 4;
coaster_ring_h = 2;          // locating ring on the lid keeps the coaster under the dripper

/* [Flow meter: measure yours] */
meter_body = [45, 30, 28];   // length along the flow x width x height, without the barbs
meter_barb_len = 12;         // each end

/* [Column, 2020 extrusion] */
ext = 20;
column_socket_depth = 35;   // leaves solid plastic under the column for its M5 bolt

/* [Head: theta motor + pivot] */
nema = 42.3;
nema_hole_spacing = 31;
nema_boss_d = 22.5;          // flange pilot
coupler_d = 20;              // 5 mm to 8 mm flexible coupler
coupler_len = 25;
collar_t = 10;               // 8 mm shaft collar above the top bearing
bearing_od = 22;             // 608
bearing_t = 7;
head_h = 65;
head_gap = 2;                // between head and arm hub

/* [Arm] */
// Nothing on the arm may reach further than ~22 mm behind the pivot, or it
// hits the column when the arm swings.
shaft_d = 8;
hub_d = 26;
hub_h = 20;
finger_r = 21;               // theta endstop finger, points at the column at theta = 0
beam_t = 8;
beam_back = -15;
beam_front = 95;
beam_y = [-16, 46];          // beam spans the rail (y = 0) and the motor/belt side
rail_len = 200;
rail_start = 20;             // rail end nearest the pivot
rail_w = 12;
rail_h = 8;
rail_hole_pitch = 25;
rail_hole_first = 12.5;
radial_motor_x = 45;         // pancake NEMA17 on top of the beam, shaft down
radial_motor_len = 23;       // pancake NEMA17 body
radial_motor_pocket = 4;     // motor face sits this far into the beam (pulley mounted hub-up)
belt_y = 24;                 // belt loop centre line, beside the carriage
pulley_z = [-16, -2];        // pulley/idler band below the beam
pulley_pitch_r = 6.37;       // GT2 20T

/* [Carriage + nozzle (MGN12H)] */
carriage_w = 27;
carriage_len = 45.4;
carriage_h = 13;             // rail mount face to carriage face
carriage_hole_spacing = 20;  // 20 x 20 M3 pattern
nozzle_d = 8;                // leg of a 1/4" stainless elbow barb
nozzle_drop = 25;            // nozzle tip below the holder plate
holder_t = 6;

/* [Endstops: KW11-3Z style lever switch, 20 x 10 x 6.4] */
switch_hole_spacing = 9.5;
switch_hole_d = 2.2;

// ---------------------------------------------------------------- derived
lid_top = tub_height + lid_t;
cup_base_z = lid_top + coaster_t;
nozzle_tip_z = cup_base_z + cup_height + dripper_height + nozzle_clearance;
beam_bottom_z = nozzle_tip_z + nozzle_drop + holder_t + carriage_h;
head_bottom_z = beam_bottom_z + hub_h + head_gap;
head_top_z = head_bottom_z + head_h;
column_bottom_z = tub_height - column_socket_depth;
column_len = ceil((head_top_z - column_bottom_z) / 10) * 10;  // cut your 2020 to this
