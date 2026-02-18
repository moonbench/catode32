# Virtual Pet

A virtual pet game for ESP32 with MicroPython, featuring an SSD1306 OLED display and button controls.

![catstars](https://github.com/user-attachments/assets/2ffc652a-f392-42e7-9a13-d7fb91f3770d)

## Setup

### Hardware Requirements

- **ESP32-C6 SuperMini** development board
- **SSD1306 OLED Display** (128x64, I2C)
- **8 Push Buttons** for input

### Software Requirements

- `mpremote` installed (`pip install mpremote`)

### Wiring

This is the wiring I used for the project. If you change these, then you'll want to update the values in `src/config.py`

**Display (I2C):**
|Display Pin | ESP32-C6 Pin |
|--------|----------|
|VCC | 3V3 |
|GND | GND |
|SDA | GPIO4 |
|SCL | GPIO7 |

**Buttons:**
| Button | GPIO Pin |
|--------|----------|
| UP     | GPIO0    |
| DOWN   | GPIO1    |
| LEFT   | GPIO2    |
| RIGHT  | GPIO3    |
| A      | GPIO20   |
| B      | GPIO19   |
| MENU1  | GPIO18   |
| MENU2  | GPIO14   |

Each button connects between GPIO pin and GND (internal pull-ups enabled).

## Installation

### 1. Flash MicroPython (if not already done)

```bash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* erase_flash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* write_flash -z 0x0 ESP32_GENERIC_C6-*.bin
```

### 2. Install SSD1306 Library

```bash
mpremote mip install ssd1306
```

## Development Workflow

For the fastest iteration during development, use the `dev.sh` script which compiles Python to bytecode and runs via `mpremote mount`:

```bash
./dev.sh
```

This script:
- Compiles all `.py` files in `src/` to `.mpy` bytecode in `build/`
- Mounts the `build/` directory on the device
- Runs the game

Using precompiled `.mpy` files provides faster startup and slightly lower RAM usage compared to raw `.py` files.

> [!NOTE]
> Requires `mpy-cross` (`pip install mpy-cross`) and `mpremote` (`pip install mpremote`).
> Any libraries used (like `ssd1306`) must already be installed on the device.

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
