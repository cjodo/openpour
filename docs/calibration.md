# First run and calibration

Everything here is done from the **Machine** tab of the app. Values are saved
on the machine and survive firmware updates.

## 1. Connect

On first boot the machine starts its own Wi-Fi network, **OpenPour-XXXX**,
with the password `pourover`. Join it and open http://192.168.4.1. Under
**Wi-Fi**, enter your home network and save. The machine restarts and is
then reachable at http://openpour.local. On some Android devices `.local`
names don't resolve, so use the IP address shown in the serial monitor or
your router's client list.

Add the page to your home screen to use it like an app.

## 2. Check directions and endstops

1. Press **Home**. The carriage should travel toward the pivot and stop on
   its switch, then the arm should swing toward its switch.
   - If an axis moves the wrong way, press **Motors off**, open
     **Advanced**, tick the matching **Reverse … direction** box, save, and
     home again.
   - If a switch never triggers, homing stops after 20 s with an error.
2. The arm then parks out of the way of the dripper.

## 3. Set the dripper centre

Put your dripper on the platform. Press **Go to centre**, then use the jog
pad until the nozzle sits over the middle of the dripper. Use **Fine** for
the last millimetres. Press **Save as centre**. Every pour pattern is drawn
around this point, so it absorbs any slop in the printed parts.

## 4. Calibrate the scale

1. Clear the platform and press **Tare**.
2. Place something of known weight on it. A 500 g bag of coffee or a glass
   of water weighed on a kitchen scale works.
3. Enter the weight and press **Calibrate scale**. Keep it still for a
   moment while the machine averages the readings.

Check by adding and removing a few known items.

## 5. Prime and calibrate the pump

1. Fill the reservoir with hot water and put a cup on the platform.
2. Press **Prime 3 s** a few times until water flows steadily from the
   nozzle.
3. Empty the cup, put it back and press **Calibrate pump**. The machine
   homes, centres, tares, and runs the pump flat out for 10 s. The measured
   rate is stored and used to plan every pour.

## 6. Brew

On the **Brew** tab, choose a recipe and press **Start brew**, or press the
button on the base to brew the last recipe again. A short press pauses or
resumes, and a long press stops.

## Tuning notes

| Symptom | Setting (Advanced) |
|---|---|
| Overshoots each stage target | Raise **Pour stop lead time** |
| Stops short of each target | Lower **Pour stop lead time** |
| Pump stalls at slow flow rates | Raise **Pump minimum power** |
| Pattern looks small or large | Re-check **Carriage steps per mm** (80 for GT2 20T at 1/16) |
| Arm hits something at the end of travel | Narrow the arm/carriage min/max limits |
