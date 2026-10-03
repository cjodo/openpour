# Assembly

![Assembly preview](images/assembly.png)

Before you start, set your cup and dripper heights in
`hardware/cad/config.scad`. `column_len` (printed in the OpenSCAD console as
a derived value) is the length to cut the 2020 extrusion to. A 400 mm stock
length covers mugs up to ~120 mm under a V60.

## Base

1. Press rubber feet into the four recesses under `base_tub`.
2. Tap the 2020 extrusion's centre hole M5 at one end. Push it into the
   socket and bolt it from underneath with an M5 × 12.
3. Screw the load cell's **fixed end** to the pedestal from below (two M4).
   Its arrow should point down, and the free end should point toward the
   front.
4. Fit the ESP32 + HX711 prototype board on the standoffs, the stepper
   carriers on either side, and the MOSFET module and buck converter beside
   the load cell. Fix them with double-sided foam tape. Fit the DC jack to
   the rear wall. See [wiring.md](wiring.md).
5. Fit the button to the lid, route the cable bundle through the slot beside
   the column, and screw the lid on (4 × M3, countersunk).
6. Lower the platform's post through the lid. Bolt it to the cell's free
   end with two M4 × 10 dropped down the bores. The platform must not touch
   the lid anywhere. Drop the coaster into its recess.

## Head and arm

1. Press the two 608 bearings into the head: one from below, and one into
   the pocket under the coupler cavity.
2. Bolt the 40 mm NEMA17 face-down on top of the head (4 × M3 from below).
   Fit the coupler on its shaft.
3. Slide the 8 mm shaft up through both bearings into the coupler. Put the
   collar on the shaft above the top bearing first, then tighten the
   coupler. The collar takes the arm's weight. Snug it so the shaft has no
   end-play but still turns freely.
4. Slide the head down the column on four T-nuts. Set its height so the
   nozzle ends up ~20 mm above your dripper rim, then tighten.
5. Fix the rail to the underside of `arm_root` (M3 up through the rail into
   nuts in the hex traps). Bolt `arm_tip` to the last rail hole.
6. Bolt the pancake motor into its pocket and fit the GT2 pulley hub-up
   below the beam. Fit the idler in the arm tip slot on an M5 × 40.
7. Clamp the arm's hub onto the shaft below the head with the M3 clamp
   screw.
8. Bolt `nozzle_holder` under the carriage (4 × M3). Loop the belt around
   the pulley and idler, tuck both ends into the holder's slot, pinch them
   with the M3, and tension by sliding the idler.
9. Fit one `switch_bracket` under the beam (radial endstop) so the carriage
   presses the lever about 23 mm before its centre reaches 66 mm from the
   pivot. Fit the other under the head (theta endstop) so the hub's finger
   presses it as the arm swings to about −75°. Fine-tune the exact numbers
   with **Arm angle at endstop** and **Carriage radius at endstop** in
   Advanced settings.

## Water

1. Bolt the pump bracket to the column with two T-nuts, then fit the pump.
2. Push the stainless elbow's vertical leg into the nozzle holder so it pokes
   out ~25 mm below, then pinch it.
3. Run silicone tube from the bottom of the reservoir (weight the end with a
   stainless washer) through the pump and up to the elbow. Leave a loose loop
   at the arm so it can swing.
4. Hang the DS18B20 probe in the reservoir.

Then work through [calibration.md](calibration.md).
