#!/bin/sh
# Builds everything the Wokwi simulation needs:
#   chips/*.chip.wasm  the pump + flow meter and the stepper/endstop chips
#   openpour.bin       a full 4 MB flash image (bootloader, partition table and
#                      the firmware built with --features wokwi), so LittleFS
#                      and the core dump partition exist as on a real board
set -e
here=$(cd "$(dirname "$0")" && pwd)

cd "$here/chips"
cargo build --release
for chip in flowmeter endstop; do
  cp "target/wasm32-unknown-unknown/release/$chip.wasm" "$chip.chip.wasm"
done

cd "$here/../esp32"
[ -f "$HOME/export-esp.sh" ] && . "$HOME/export-esp.sh"
cargo build --release --features wokwi
espflash save-image --chip esp32 --merge --flash-size 4mb --partition-table partitions.csv \
  target/xtensa-esp32-espidf/release/openpour "$here/openpour.bin"
echo "Built $here/openpour.bin. Run: wokwi-cli $here"
