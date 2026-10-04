# Bill of materials

Every purchased part is a commodity item from the 3D-printer, maker or
plumbing aisle. All of them are available from AliExpress, Amazon and most
maker shops. Prices are rough single-unit figures as a guide, not quotes.
CAD and EUR are converted from USD at 1 USD ≈ 1.42 CAD ≈ 0.89 EUR
(October 2026), so local prices and shipping will vary.

## Motion

| Qty | Part | Notes | ≈ USD | ≈ CAD | ≈ EUR |
|---|---|---|---|---|---|
| 1 | NEMA17 stepper, 40 mm body | Arm swing (theta), direct drive | 10 | 14 | 9 |
| 1 | NEMA17 "pancake" stepper, 20–23 mm body | Carriage (radial). Shaft ≥ 18 mm | 9 | 13 | 8 |
| 1 | MGN12H linear rail, 200 mm, with carriage | The arm itself | 12 | 17 | 11 |
| 1 | GT2 20T pulley, 5 mm bore, 6 mm belt | On the pancake motor, mounted hub-up | 1 | 1 | 1 |
| 1 | GT2 20T toothless idler, 5 mm bore | Arm tip | 1 | 1 | 1 |
| 1 | GT2 6 mm open belt, 1 m | ~520 mm used | 2 | 3 | 2 |
| 1 | Flexible shaft coupler, 5 mm to 8 mm | Theta motor to pivot shaft | 2 | 3 | 2 |
| 1 | 8 mm steel rod, 100 mm | Pivot shaft (linear rod offcut is fine) | 3 | 4 | 3 |
| 2 | 608ZZ bearing | Pivot | 1 | 1 | 1 |
| 1 | 8 mm shaft collar | Above the top bearing, carries the arm's weight | 1 | 1 | 1 |
| 1 | 2020 aluminium extrusion, 400 mm | Column. `column_len` in the CAD gives the exact length for your cups | 5 | 7 | 4 |
| 2 | Lever microswitch, KW11-3Z (20 × 10 mm) | Endstops | 1 | 1 | 1 |

## Water

| Qty | Part | Notes | ≈ USD | ≈ CAD | ≈ EUR |
|---|---|---|---|---|---|
| 1 | 12 V peristaltic pump, 300–400 mL/min | Kamoer KPHM400-style or equivalent. Peristaltic means only the tube touches water | 25–40 | 36–57 | 22–36 |
| 1.5 m | Food-grade silicone tubing to fit the pump, rated ≥ 100 °C | | 4 | 6 | 4 |
| 1 | ¼" (6 mm) 304 stainless 90° elbow hose barb | The nozzle. One leg is clamped vertically by the holder | 3 | 4 | 3 |
| 1 | Vacuum-insulated jug, 1–1.5 L, wide mouth | The reservoir. Fill it from your kettle | 15 | 21 | 13 |
| 1 | DS18B20 waterproof probe (stainless) + 4.7 kΩ resistor | Hangs in the reservoir | 2 | 3 | 2 |
| 1 | Hall-effect flow meter for hot drinks + 10 kΩ resistor | Measures the water, since 1 mL = 1 g. See below | 20–35 | 28–50 | 18–31 |
| 1 | Cork or silicone coaster, 95 mm | Sits in the lid's ring so hot cups never touch the print | 1 | 1 | 1 |

### Choosing the flow meter

The machine pours by volume, so the flow meter sets the accuracy.
- **Food-safe and rated for brewing-temperature water (≥ 90 °C).** Meters
  made for coffee machines, such as the Digmesa FHKSC family, are built for
  this.
- **Accurate from about 60 to 500 mL/min,** which covers recipes from 1 to
  8 g/s.
- **Pulse output with a known pulses-per-litre figure.** It's usually
  open-collector, powered from 5 V. The firmware's default is 1925 pulses/L;
  calibration measures yours.
- **Barbs that fit your tubing,** usually ¼" (6 mm).

Avoid the cheap YF-S401 / YF-S201 garden-style sensors. Most are rated to
about 80 °C, are not food-safe, and lose accuracy below about 300 mL/min.

## Electronics

| Qty | Part | Notes | ≈ USD | ≈ CAD | ≈ EUR |
|---|---|---|---|---|---|
| 1 | ESP32 DevKit V1 (ESP32-WROOM-32) | 30- or 38-pin. Not a WROVER board (GPIO 16) | 5 | 7 | 4 |
| 2 | TMC2209 StepStick driver | Standalone STEP/DIR, MS1 + MS2 high for 1/16 microstepping | 8 | 11 | 7 |
| 2 | StepStick carrier/expansion board | The small red boards with screw terminals and a 100 µF cap | 2 | 3 | 2 |
| 1 | Logic-level MOSFET module (AOD4184 / D4184 type) | Pump PWM | 1 | 1 | 1 |
| 1 | Schottky diode SS34 or 1N5819 | Across the pump motor (flyback) | 0.2 | 0.3 | 0.2 |
| 1 | Buck converter 12 V → 5 V (MP1584 or LM2596) | Powers the ESP32 via its 5 V/VIN pin | 1 | 1 | 1 |
| 1 | 12 V 5 A power supply, 5.5 × 2.1 mm plug | | 10 | 14 | 9 |
| 1 | Panel-mount DC jack, 5.5 × 2.1 mm (11–12 mm hole) | | 1 | 1 | 1 |
| 1 | 12 mm momentary push button | Start / pause / stop | 1 | 1 | 1 |
| 1 | 5 × 7 cm double-sided prototype board + female headers | Carries the ESP32 | 1 | 1 | 1 |
| – | Dupont/JST leads, 22 AWG wire for 12 V | | 3 | 4 | 3 |

## Fasteners

| Qty | Part | Where |
|---|---|---|
| ~30 | M3 screws 8–12 mm + nuts | Lid, rail, carriage, brackets, motors (from below through the motor plates) |
| 1 | M5 × 12 mm | Up through the base into the column (tap the extrusion's centre hole M5) |
| 4 | M5 × 10 mm + 4 drop-in T-nuts | Head clamp |
| 2 | M5 × 10 mm + 2 drop-in T-nuts | Pump bracket |
| 2 | M5 × 10 mm + 2 drop-in T-nuts | Flow meter clip |
| 2 | Cable ties, 3–4 mm wide | Flow meter to its clip |
| 1 | M5 × 40 mm + nut | Idler axle |
| 4 | 10 mm self-adhesive rubber feet | Base |

Expect about **US$185–235 / CA$260–330 / €165–210** total with the pump
and jug, less if you have printer leftovers.

## Printed parts

Export with `make` in `hardware/cad` (or open `openpour.scad` and pick a
`part`). Print in PETG or ASA at 0.2 mm, 4 perimeters, 30 % infill.

| Part | Qty | Orientation |
|---|---|---|
| base_tub | 1 | as exported (floor down) |
| base_lid | 1 | as exported |
| head | 1 | as exported (bearing pocket down) |
| arm_root | 1 | as exported (rail face down) |
| arm_tip | 1 | as exported |
| nozzle_holder | 1 | as exported. Supports under the belt clamp |
| switch_bracket | 2 | as exported |
| pump_bracket | 1 | as exported. Drill the slots to suit your pump |
| meter_clip | 1 | as exported (column plate down). Set `meter_body` in `config.scad` to your meter first |

No printed part touches water or a hot cup. Water only touches the silicone
tube, the flow meter and the stainless nozzle, and the coaster isolates the
cup's heat.
PETG softens around 80 °C, which is why the coaster matters.
