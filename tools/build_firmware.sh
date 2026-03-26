#!/bin/bash
# Build MicroPython firmware for petpython with frozen assets.
#
# Usage:
#   ./tools/build_firmware.sh [esp32c6|esp32c3]   (default: esp32c6)
#   ./tools/build_firmware.sh esp32c3
#
# Requirements:
#   - ESP-IDF at ~/esp/esp-idf (or set IDF_PATH)
#   - MicroPython at ~/esp/micropython (or set MICROPYTHON_DIR)

set -e

BOARD="${1:-esp32c6}"
case "$BOARD" in
    esp32c6) MICROPY_BOARD="ESP32_GENERIC_C6" ;;
    esp32c3) MICROPY_BOARD="ESP32_GENERIC_C3" ;;
    *) echo "Unknown board: $BOARD. Use esp32c6 or esp32c3."; exit 1 ;;
esac

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
IDF_PATH="${IDF_PATH:-$HOME/esp/esp-idf}"
MICROPYTHON_DIR="${MICROPYTHON_DIR:-$HOME/esp/micropython}"

echo "Building for $MICROPY_BOARD"
echo "Project: $PROJECT_DIR"
echo "ESP-IDF: $IDF_PATH"
echo "MicroPython: $MICROPYTHON_DIR"

# Activate ESP-IDF environment
. "$IDF_PATH/export.sh"

# Export project src path for manifest.py
export PETPYTHON_SRC="$PROJECT_DIR/src"

cd "$MICROPYTHON_DIR/ports/esp32"
idf.py \
    -D MICROPY_BOARD="$MICROPY_BOARD" \
    -D MICROPY_FROZEN_MANIFEST="$PROJECT_DIR/manifest.py" \
    build

echo ""
echo "Firmware: $MICROPYTHON_DIR/ports/esp32/build/micropython.bin"
echo ""
echo "To flash (replace PORT with your device port, e.g. /dev/tty.usbmodem*):"
echo "  python -m esptool --chip $BOARD -p PORT -b 460800 \\"
echo "    --before default_reset --after hard_reset write_flash \\"
echo "    --flash_mode dio --flash_size 4MB --flash_freq 80m \\"
echo "    0x0 build/bootloader/bootloader.bin \\"
echo "    0x8000 build/partition_table/partition-table.bin \\"
echo "    0x10000 build/micropython.bin"
