# Virtual Pet

A virtual pet game for ESP32 with MicroPython, featuring an SSD1306 OLED display and button controls.

## Prerequisites

- ESP32 flashed with MicroPython
- `mpremote` installed (`pip install mpremote`)

## Development Workflow

For the fastest iteration during development, use `mpremote mount` to run code directly from your laptop without writing to flash:

```bash
mpremote mount src run src/main.py
```

This mounts your local `src/` directory as the device's filesystem and executes `main.py`. All imports resolve from your laptop, so edits take effect immediately on the next run. Nothing is written to flash memory.

To run a single script without mounting (useful for quick tests):

```bash
mpremote run src/main.py
```

Note: Any libraries the script imports (like `ssd1306`) must already be installed on the device.

## Scripts

### test_hardware.sh

Verifies that your hardware is working correctly:

```bash
./test_hardware.sh
```

This script:
- Resets the device
- Scans I2C to confirm the display is detected
- Enters an interactive button test (press buttons to see them register, Ctrl+C to exit)

Run this first when setting up a new device or debugging hardware issues.

### upload.sh

Deploys the project to the ESP32's flash storage:

```bash
./upload.sh [port]
```

This script:
- Installs the `ssd1306` library via `mip`
- Cleans existing files from the device (preserves `boot.py` and `lib/`)
- Uploads all Python files from `src/` to the device

Use this when you want the pet to run standalone without a laptop connection.

## Running the Game

After uploading, start the game with:

```bash
mpremote exec 'import main; main.main()'
```

Or connect to the REPL and run interactively:

```bash
mpremote
>>> import main
>>> main.main()
```

## Controls

- **D-pad**: Navigate / Move character
- **A/B buttons**: Action buttons
- **Menu buttons**: Additional functions
