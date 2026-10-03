# OpenPour

An open-source automatic pour-over machine. It has a 3D-printed body and
commodity parts from the 3D-printer aisle. It's controlled from any phone or
laptop browser, with no app store and no cloud.

![Assembly preview](docs/images/assembly.png)

- **Pours like a barista.** A two-axis polar arm traces centre, circle and
  spiral patterns over the dripper.
- **Pours by weight.** A load cell under the cup closes the loop, so each
  stage stops at its gram target whatever the pump does.
- **Configurable recipes.** Stages, water, flow rate, pattern, radius, speed
  and bloom/wait times are all editable in the app and stored on the machine.
- **Low voltage only.** You fill an insulated reservoir from your own
  kettle. The machine pumps, weighs and measures temperature, but never
  heats water.
- **Parametric.** Set your cup and dripper heights in one OpenSCAD file and
  the column length, head position and parts follow.

## How it works

```
 reservoir ──tube──► peristaltic pump ──► nozzle on carriage
 (hot water,                                │  radial axis (belt, pancake NEMA17)
  DS18B20)                                  │
                                 arm ───────┘  theta axis (direct-drive NEMA17)
                                  │
                       dripper on a load-cell platform (HX711)
                                  │
                ESP32 ◄───────────┘ ──Wi-Fi──► browser app (served by the ESP32)
```

The firmware converts each pattern from dripper-centred coordinates into arm
angle and carriage radius 100 times a second. Both steppers run in velocity
mode so the nozzle can follow spirals smoothly. The pump's speed is
feed-forward from its calibrated rate, trimmed by the measured flow, and cut
early enough that the water still in flight lands on target.

## Repository

| Path | What |
|---|---|
| `hardware/cad/` | OpenSCAD model. `config.scad` holds every dimension; `make` exports STLs |
| `firmware/` | ESP32 firmware (PlatformIO, Arduino framework) |
| `web/` | The control app: plain HTML/CSS/JS, embedded into the firmware at build time |
| `docs/` | [BOM](docs/BOM.md), [wiring](docs/wiring.md), [assembly](docs/assembly.md), [calibration](docs/calibration.md) |

## Build it

1. **Print.** In `hardware/cad`, edit `config.scad`, then run `make`, or
   open `openpour.scad` in OpenSCAD and choose a part. Print in PETG or ASA.
2. **Buy.** See the [bill of materials](docs/BOM.md). It comes to about
   $170–210.
3. **Wire.** See [wiring](docs/wiring.md).
4. **Flash.** Install [PlatformIO](https://platformio.org), then:
   ```sh
   cd firmware
   pio run -t upload      # firmware + web app in one image
   pio device monitor     # shows the address to open
   ```
5. **Calibrate.** Follow [calibration](docs/calibration.md). On first boot,
   join the `OpenPour-XXXX` Wi-Fi network (password `pourover`) and open
   http://192.168.4.1.

## Develop the app without hardware

```sh
cd web
python3 -m http.server 8000
# open http://localhost:8000/?mock          (add &speed=4 to fast-forward)
```

`mock.js` simulates the machine with the same REST and WebSocket protocol as
the firmware, so recipes, brewing and calibration all work in the browser.

## Firmware tests

The kinematics, pour patterns and flow control are plain C++ with no
hardware dependencies:

```sh
cd firmware
pio test -e native
```

## Recipe format

Recipes are stored on the machine as JSON. `water` is the amount added
in that stage, in grams.

```json
{
  "id": "v60-single",
  "name": "V60 single cup",
  "dose": 15,
  "minTemp": 92,
  "stages": [
    { "name": "Bloom", "water": 45, "flow": 4, "pattern": "spiral", "radius": 22, "rps": 1.2, "wait": 40 },
    { "name": "Pour 1", "water": 55, "flow": 4.5, "pattern": "spiral", "radius": 26, "rps": 1, "wait": 10 }
  ]
}
```

`pattern` is `center`, `circle` or `spiral`. `rps` is revolutions per
second around the dripper.

## API

| | |
|---|---|
| `GET /api/settings`, `POST /api/settings` | Machine settings (partial updates allowed) |
| `GET /api/recipes`, `POST /api/recipes` | The full recipe array |
| `ws://<host>/ws` | Status pushed 5× per second; send `{"cmd": "start", "recipe": "<id>"}`, `pause`, `resume`, `stop`, `tare`, `home`, `park`, `center`, `jog`, `setCenter`, `release`, `prime`, `calScale`, `calPump` |

## Status

The firmware compiles and its core logic is unit-tested. The app runs
against the simulator. The mechanical design is a first revision that hasn't
been printed yet. Expect to adjust clearances, switch positions and hole
sizes on the first build. Reports and fixes are welcome.

## Safety

Hot water burns. Keep the reservoir lid on and the machine on a stable
surface, and never run the pump without a cup on the platform. The firmware
stops if no water reaches the scale or if the cup would overflow, but it is
not a substitute for paying attention.

## License

MIT. See [LICENSE](LICENSE).
