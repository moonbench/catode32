x!/bin/bash
# Build and/or flash MicroPython firmware for petpython with frozen assets.
#
# Usage:
#   ./tools/build_firmware.sh [build|flash|build-flash] [esp32c6|esp32c3] [port]
#
#   build              - build only (default)
#   flash              - flash only (firmware must already be built)
#   build-flash        - build then flash
#
#   board defaults to esp32c6
#   port defaults to auto-detected /dev/tty.usbmodem* or /dev/tty.SLAB*
#
# Examples:
#   ./tools/build_firmware.sh
#   ./tools/build_firmware.sh build-flash
#   ./tools/build_firmware.sh flash esp32c6 /dev/tty.usbmodem1234
#   ./tools/build_firmware.sh build esp32c3
#
# Requirements:
#   - ESP-IDF at ~/esp/esp-idf (or set IDF_PATH)
#   - MicroPython at ~/esp/micropython (or set MICROPYTHON_DIR)

set -e

CMD="${1:-build}"
BOARD="${2:-esp32c6}"
PORT="${3:-}"

case "$BOARD" in
    esp32c6) MICROPY_BOARD="ESP32_GENERIC_C6" ;;
    esp32c3) MICROPY_BOARD="ESP32_GENERIC_C3" ;;
    *) echo "Unknown board: $BOARD. Use esp32c6 or esp32c3."; exit 1 ;;
esac

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
IDF_PATH="${IDF_PATH:-$HOME/esp/esp-idf}"
MICROPYTHON_DIR="${MICROPYTHON_DIR:-$HOME/esp/micropython}"
BUILD_DIR="$MICROPYTHON_DIR/ports/esp32/build"

# Auto-detect port if not specified
detect_port() {
    for pattern in /dev/tty.usbmodem* /dev/tty.SLAB_USBtoUART* /dev/tty.wchusbserial* /dev/tty.usbserial*; do
        local matches=($pattern)
        if [ -e "${matches[0]}" ]; then
            echo "${matches[0]}"
            return
        fi
    done
    echo ""
}

do_build() {
    echo "Building for $MICROPY_BOARD"
    echo "Project:     $PROJECT_DIR"
    echo "ESP-IDF:     $IDF_PATH"
    echo "MicroPython: $MICROPYTHON_DIR"
    echo ""

    . "$IDF_PATH/export.sh"
    export PETPYTHON_SRC="$PROJECT_DIR/src"

    cd "$MICROPYTHON_DIR/ports/esp32"
    idf.py \
        -D MICROPY_BOARD="$MICROPY_BOARD" \
        -D MICROPY_FROZEN_MANIFEST="$PROJECT_DIR/manifest.py" \
        build

    echo ""
    echo "Firmware: $BUILD_DIR/micropython.bin"
}

do_flash() {
    local port="${1:-}"
    if [ -z "$port" ]; then
        port=$(detect_port)
    fi
    if [ -z "$port" ]; then
        echo "Error: no device found. Connect your ESP32 and try again, or specify the port:"
        echo "  $0 flash $BOARD /dev/tty.your-port"
        exit 1
    fi

    echo "Flashing to $port ($BOARD)..."
    . "$IDF_PATH/export.sh" 2>/dev/null

    python -m esptool --chip "$BOARD" -p "$port" -b 460800 \
        --before default_reset --after hard_reset write_flash \
        --flash_mode dio --flash_size 4MB --flash_freq 80m \
        0x0     "$BUILD_DIR/bootloader/bootloader.bin" \
        0x8000  "$BUILD_DIR/partition_table/partition-table.bin" \
        0x10000 "$BUILD_DIR/micropython.bin"

    echo ""
    echo "Done. Your device is running the new firmware."
    echo ""
    echo "Verify frozen assets at the REPL:"
    echo "  import micropython, assets.character"
    echo "  micropython.mem_info()"
}

case "$CMD" in
    build)       do_build ;;
    flash)       do_flash "$PORT" ;;
    build-flash) do_build && do_flash "$PORT" ;;
    *) echo "Unknown command: $CMD. Use build, flash, or build-flash."; exit 1 ;;
esac
