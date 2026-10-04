# Wiring

Everything runs at 12 V or below. There is no mains voltage inside the
machine. The power brick is the only mains part, and it is off-the-shelf.

## Power

```
12 V brick ── DC jack ──┬── stepper carrier (theta)  VMOT
                        ├── stepper carrier (radial) VMOT
                        ├── pump (+) ──── pump (−) ── MOSFET module OUT−
                        │                 (SS34 across the pump, stripe to +)
                        ├── MOSFET module VIN+
                        └── buck converter IN ── 5 V OUT ── ESP32 5V/VIN
GND is common to everything, including the ESP32.
```

Set the buck converter to 5.0 V **before** connecting the ESP32.

## ESP32 pin map

The pin map is defined at the top of `firmware/esp32/src/main.rs`.

| ESP32 GPIO | Connects to | Notes |
|---|---|---|
| 26 | Theta driver STEP | |
| 25 | Theta driver DIR | |
| 33 | Radial driver STEP | |
| 32 | Radial driver DIR | |
| 27 | Both drivers EN | Active low, shared |
| 18 | Theta endstop | Switch COM to GND, NO to the pin (internal pull-up) |
| 19 | Radial endstop | Switch COM to GND, NO to the pin |
| 16 | Flow meter pulse output | 10 kΩ from the pin to 3V3. WROOM modules only, since WROVER uses 16 for PSRAM |
| 23 | MOSFET module PWM/SIG | |
| 4 | DS18B20 data | 4.7 kΩ from data to 3V3 |
| 13 | Push button | Other leg to GND |
| 2 | On-board LED | Status |

Power the DS18B20 from **3V3**, not 5 V, so its output is safe for the
ESP32. The flow meter can run from 5 V because its output is open-collector.
Pull that output up to **3V3 only**. If your meter has its own pull-up to its
supply, power it from 3V3 if the datasheet allows, or add a divider
(10 kΩ / 20 kΩ) so the pin never sees 5 V.

## Stepper drivers (TMC2209, standalone)

- Microstepping: MS1 **high**, MS2 **high** gives 1/16, which the firmware
  defaults assume. On carrier boards with three DIP switches, MS3 has no
  effect on a TMC2209, but check your driver's pinout. Some variants route
  PDN/UART to that pin.
- Motor current: set Vref for about 0.6 A RMS on the 40 mm motor and 0.4 A on the
  pancake. Most TMC2209 StepSticks use Vref ≈ I<sub>RMS</sub> × 1.41, but
  confirm against your board's documentation.
- Pair motor coils with a multimeter (continuity within a coil). If an axis
  runs backwards, tick **Reverse … direction** in the app's Advanced settings
  instead of rewiring.

## Flow meter

| Meter wire (typical) | Connects to |
|---|---|
| Red | 5 V |
| Black | GND |
| White, yellow or blue (signal) | GPIO 16, with 10 kΩ to 3V3 |

Wire colours vary, so check your meter's datasheet. The meter goes in the
tube between the pump and the nozzle, with its arrow pointing in the
direction of flow.

## Cable routing

Motor, endstop, pump, probe and flow meter cables leave the base through the slot beside
the column. Bundle them up the column with spiral wrap and leave a loose loop
to the arm so it can swing from −78° to +25°. The pump tube follows the same
path, through the flow meter, to the nozzle elbow.
