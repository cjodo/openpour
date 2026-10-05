#!/bin/sh
# Exports the assembled machine as one STL per component, all in assembly
# coordinates (mm, Z up), so they import already in place and each can get its
# own material. Used by blender/scene.py; see README.md.
#
#   hardware/render/export.sh [-o DIR] [-D 'var=value' ...]
#
#   -o DIR   where to write the STLs (default hardware/render/out/assembly)
#   -D ...   passed to OpenSCAD, e.g. -D 'arm_theta=-30' -D 'carriage_r=140'
#            -D 'show_cup=false' -D '$fn=128'
set -eu

here=$(cd "$(dirname "$0")" && pwd)
scad=$here/../cad/openpour.scad
out=$here/out/assembly
defines=""

while [ $# -gt 0 ]; do
  case $1 in
    -o) out=$2; shift 2 ;;
    -D) defines="$defines -D '$2'"; shift 2 ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "export.sh: unknown option $1 (try --help)" >&2; exit 2 ;;
  esac
done

# Keep in step with the component("...") names in hardware/cad/openpour.scad.
components="base_tub base_lid coaster column meter_clip flow_meter head motor_theta bearings shaft
arm rail motor_radial pulleys carriage nozzle_holder nozzle switch mug dripper"

command -v openscad >/dev/null || { echo "export.sh: openscad not found" >&2; exit 1; }
mkdir -p "$out"
rm -f "$out"/*.stl
jobs=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)
echo "Exporting $(echo $components | wc -w) components to $out ($jobs at a time)..."

# One OpenSCAD per component. A component that renders nothing (e.g. the
# mug with show_cup=false) leaves no file.
echo $components | tr ' ' '\n' | xargs -P "$jobs" -I{} sh -c \
  "openscad -q $defines -D 'part=\"assembly\"' -D 'component=\"{}\"' -o '$out/{}.stl' '$scad' 2>/dev/null || rm -f '$out/{}.stl'"

n=$(ls "$out"/*.stl 2>/dev/null | wc -l)
echo "Wrote $n STL files. Open them in Blender with:"
echo "  blender --python $here/blender/scene.py -- $out"
