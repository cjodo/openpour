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

Put your cup and dripper on the coaster. Press **Go to centre**, then use the jog
pad until the nozzle sits over the middle of the dripper. Use **Fine** for
the last millimetres. Press **Save as centre**. Every pour pattern is drawn
around this point, so it absorbs any slop in the printed parts.

## 4. Prime

Fill the reservoir with hot water and put a cup under the nozzle. Press
**Prime 3 s** a few times until water flows steadily from the nozzle, with
no air bubbles in the tube.

The machine measures water as it passes the flow meter, so the tube between
the meter and the nozzle must already be full. Prime again whenever the
tube has drained, for example after refilling an empty reservoir.

## 5. Calibrate the flow meter

The meter's datasheet value is only a starting point. Each meter, and how
it's mounted, varies by a few percent.

1. Put an empty jug, or a mug on a kitchen scale (tared), under the nozzle.
2. Press **Dispense ~200 ml**. The machine homes, centres, and pumps until
   the meter has counted about 200 ml.
3. Measure what came out. On a kitchen scale, 1 g = 1 ml. Enter it under
   **Real amount** and press **Calibrate meter**.

Repeat once to check. The counted and real amounts should now agree to
within a couple of percent. Use hot water, as it would be when brewing.

## 6. Calibrate the pump

Empty the cup, put it back under the nozzle and press **Calibrate pump**.
The machine runs the pump flat out for 10 s and stores the rate the meter
measured. That rate is used to plan every pour.

## 7. Brew

On the **Brew** tab, choose a recipe and press **Start brew**, or press the
button on the base to brew the last recipe again. A short press pauses or
resumes, and a long press stops.

## Tuning notes

| Symptom | Setting (Advanced) |
|---|---|
| Overshoots each stage target | Raise **Pour stop lead time** |
| Stops short of each target | Lower **Pour stop lead time** |
| Cup holds more or less than the recipe | Re-run the flow meter calibration, and prime first |
| Pump stalls at slow flow rates | Raise **Pump minimum power** |
| Pattern looks small or large | Re-check **Carriage steps per mm** (80 for GT2 20T at 1/16) |
| Arm hits something at the end of travel | Narrow the arm/carriage min/max limits |
