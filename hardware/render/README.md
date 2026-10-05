# Renders

Scripts for picturing the assembled machine outside OpenSCAD. Generated files
go to `out/`, which git ignores. Add new render scripts here.

## The assembly as separate parts

```sh
hardware/render/export.sh                     # out/assembly/*.stl, ~15 s
hardware/render/export.sh -D 'arm_theta=-30' -D 'carriage_r=140' -o hardware/render/out/swung
```

This exports one STL per component, rendered in parallel. All the files
share the assembly's coordinates (millimetres, Z up), so they import already
in place, and each one can get its own material. Any `-D` option goes
straight to OpenSCAD:
- `arm_theta` (−78 to 25°) and `carriage_r` (66 to 175 mm) pose the arm;
- `show_cup=false` leaves out the mug and dripper;
- `$fn=128` gives rounder curves for close-ups.

The components are named by `component("...")` in
`hardware/cad/openpour.scad`. When you add a part to the assembly there, wrap
it the same way and add its name to the list in `export.sh`.

For a single merged STL instead (no separate materials, and touching parts
fuse together):

```sh
cd hardware/cad && openscad -D 'part="assembly"' -o assembly.stl openpour.scad
```
