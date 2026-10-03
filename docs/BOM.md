# Bill of materials

Every purchased part is a commodity item from the 3D-printer, maker or
plumbing aisle. All of them are available from AliExpress, Amazon and most
maker shops. Prices are rough single-unit USD figures as a guide, not quotes.

## Motion

| Qty | Part | Notes | ≈ USD |
|---|---|---|---|
| 1 | NEMA17 stepper, 40 mm body | Arm swing (theta), direct drive | 10 |
| 1 | NEMA17 "pancake" stepper, 20–23 mm body | Carriage (radial). Shaft ≥ 18 mm | 9 |
| 1 | MGN12H linear rail, 200 mm, with carriage | The arm itself | 12 |
| 1 | GT2 20T pulley, 5 mm bore, 6 mm belt | On the pancake motor, mounted hub-up | 1 |
| 1 | GT2 20T toothless idler, 5 mm bore | Arm tip | 1 |
| 1 | GT2 6 mm open belt, 1 m | ~520 mm used | 2 |
| 1 | Flexible shaft coupler, 5 mm to 8 mm | Theta motor to pivot shaft | 2 |
| 1 | 8 mm steel rod, 100 mm | Pivot shaft (linear rod offcut is fine) | 3 |
| 2 | 608ZZ bearing | Pivot | 1 |
| 1 | 8 mm shaft collar | Above the top bearing, carries the arm's weight | 1 |
| 1 | 2020 aluminium extrusion, 400 mm | Column. `column_len` in the CAD gives the exact length for your cups | 5 |
| 2 | Lever microswitch, KW11-3Z (20 × 10 mm) | Endstops | 1 |

## Water

| Qty | Part | Notes | ≈ USD |
|---|---|---|---|
| 1 | 12 V peristaltic pump, 300–400 mL/min | Kamoer KPHM400-style or equivalent. Peristaltic means only the tube touches water | 25–40 |
| 1.5 m | Food-grade silicone tubing to fit the pump, rated ≥ 100 °C | | 4 |
| 1 | ¼" (6 mm) 304 stainless 90° elbow hose barb | The nozzle. One leg is clamped vertically by the holder | 3 |
| 1 | Vacuum-insulated jug, 1–1.5 L, wide mouth | The reservoir. Fill it from your kettle | 15 |
| 1 | DS18B20 waterproof probe (stainless) + 4.7 kΩ resistor | Hangs in the reservoir | 2 |

## Weighing

| Qty | Part | Notes | ≈ USD |
|---|---|---|---|
| 1 | Bar load cell, 5 kg, 80 × 12.7 × 12.7 mm | Measure its holes and set `lc_holes_from_end` / `lc_hole_d` in `config.scad` | 4 |
| 1 | HX711 board **with an 80 SPS option** | Red/purple boards with a RATE jumper. Green 10 SPS-only boards work, but stopping is less precise | 2 |
| 1 | Cork or silicone coaster, 95 mm | Sits in the platform recess so hot cups never touch the print | 1 |

## Electronics

| Qty | Part | Notes | ≈ USD |
|---|---|---|---|
| 1 | ESP32 DevKit V1 (ESP32-WROOM-32) | 30- or 38-pin. Not a WROVER board (GPIO 16/17) | 5 |
| 2 | TMC2209 StepStick driver | Standalone STEP/DIR, MS1 + MS2 high for 1/16 microstepping | 8 |
| 2 | StepStick carrier/expansion board | The small red boards with screw terminals and a 100 µF cap | 2 |
| 1 | Logic-level MOSFET module (AOD4184 / D4184 type) | Pump PWM | 1 |
| 1 | Schottky diode SS34 or 1N5819 | Across the pump motor (flyback) | 0.2 |
| 1 | Buck converter 12 V → 5 V (MP1584 or LM2596) | Powers the ESP32 via its 5 V/VIN pin | 1 |
| 1 | 12 V 5 A power supply, 5.5 × 2.1 mm plug | | 10 |
| 1 | Panel-mount DC jack, 5.5 × 2.1 mm (11–12 mm hole) | | 1 |
| 1 | 12 mm momentary push button | Start / pause / stop | 1 |
| 1 | 5 × 7 cm double-sided prototype board + female headers | Carries the ESP32 and HX711 | 1 |
| – | Dupont/JST leads, 22 AWG wire for 12 V | | 3 |

## Fasteners

| Qty | Part | Where |
|---|---|---|
| ~30 | M3 screws 8–12 mm + nuts | Lid, rail, carriage, brackets, motors (from below through the motor plates) |
| 4 | M4 × 10 mm | Load cell (2 from below, 2 from above). Use M5 if your cell has M5 threads |
| 1 | M5 × 12 mm | Up through the base into the column (tap the extrusion's centre hole M5) |
| 4 | M5 × 10 mm + 4 drop-in T-nuts | Head clamp |
| 2 | M5 × 10 mm + 2 drop-in T-nuts | Pump bracket |
| 1 | M5 × 40 mm + nut | Idler axle |
| 4 | 10 mm self-adhesive rubber feet | Base |

Expect about **$170–210** total with the pump and jug, less if you have
printer leftovers.

## Printed parts

Export with `make` in `hardware/cad` (or open `openpour.scad` and pick a
`part`). Print in PETG or ASA at 0.2 mm, 4 perimeters, 30 % infill.

| Part | Qty | Orientation |
|---|---|---|
| base_tub | 1 | as exported (floor down) |
| base_lid | 1 | as exported |
| platform | 1 | as exported (coaster face down) |
| head | 1 | as exported (bearing pocket down) |
| arm_root | 1 | as exported (rail face down) |
| arm_tip | 1 | as exported |
| nozzle_holder | 1 | as exported. Supports under the belt clamp |
| switch_bracket | 2 | as exported |
| pump_bracket | 1 | as exported. Drill the slots to suit your pump |

No printed part touches water or a hot cup. Water only touches the silicone
tube and the stainless nozzle, and the coaster isolates the cup's heat.
PETG softens around 80 °C, which is why the coaster matters.
