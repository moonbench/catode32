# Virtual Pet

A virtual pet game for ESP32 with MicroPython, featuring an SSD1306 OLED display and button controls.

![catstars](https://github.com/user-attachments/assets/2ffc652a-f392-42e7-9a13-d7fb91f3770d)

## Setup

### Hardware Requirements

- **ESP32-C6 SuperMini** OR **ESP32-C3** development board
- **SSD1306 OLED Display** (128x64, I2C)
- **8 Push Buttons** for input

### Software Requirements

- `mpremote` installed (`pip install mpremote`)

### Board Configuration

The project supports both ESP32-C6 and ESP32-C3 boards. To configure for your board:

1. Open `src/config.py`
2. Set `BOARD_TYPE` to either `"ESP32-C6"` or `"ESP32-C3"`

```python
# In src/config.py
BOARD_TYPE = "ESP32-C6"  # Change to "ESP32-C3" for ESP32-C3 board
```

### Wiring

Choose the wiring diagram for your board. Each button connects between GPIO pin and GND (internal pull-ups enabled).

#### ESP32-C6 Wiring

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

#### ESP32-C3 Wiring

**Display (I2C):**
|Display Pin | ESP32-C3 Pin |
|--------|----------|
|VCC | 3V3 |
|GND | GND |
|SDA | GPIO6 |
|SCL | GPIO7 |

**Buttons:**
| Button | GPIO Pin |
|--------|----------|
| UP     | GPIO0    |
| DOWN   | GPIO1    |
| LEFT   | GPIO2    |
| RIGHT  | GPIO3    |
| A      | GPIO4    |
| B      | GPIO5    |
| MENU1   | GPIO10  |
| MENU2   | GPIO11  |

> **Note:** The ESP32-C3 configuration avoids strapping pins (GPIO8, GPIO9) to prevent boot issues.

## Installation

### 1. Flash MicroPython (if not already done)

**For ESP32-C6:**
```bash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* erase_flash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* write_flash -z 0x0 ESP32_GENERIC_C6-*.bin
```

**For ESP32-C3:**
```bash
esptool.py --chip esp32c3 --port /dev/cu.usbmodem* erase_flash
esptool.py --chip esp32c3 --port /dev/cu.usbmodem* write_flash -z 0x0 ESP32_GENERIC_C3-*.bin
```

> Download MicroPython firmware from [micropython.org/download](https://micropython.org/download/)

### 2. Configure Board Type

Before uploading, set your board type in `src/config.py`:
```python
BOARD_TYPE = "ESP32-C6"  # or "ESP32-C3"
```

### 3. Install SSD1306 Library

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
- Compiles all `.py` files to `.mpy` bytecode
- Cleans existing files from the device (preserves `lib/`)
- Uploads compiled `.mpy` files and `boot.py` to the device

Use this when you want the pet to run standalone without a laptop connection.

## Running the Game

After uploading, the game starts automatically on power-up or reset.

**To enter REPL mode instead:** Hold **A+B buttons** while powering on or pressing reset. This skips auto-run so `mpremote` can connect.

To manually start the game from REPL:

```bash
mpremote
>>> import main
>>> main.main()
```

## Troubleshooting

### "could not enter raw repl" error

If you see `mpremote.transport.TransportError: could not enter raw repl` when running `./dev.sh` or other mpremote commands, it means `boot.py` is on the device and auto-running the game, blocking mpremote from connecting.

**To fix this:**

1. Run `mpremote` to connect to the device
2. Press **Ctrl+C** to interrupt the running game
3. Press **Ctrl+B** to exit raw REPL and enter friendly REPL
4. Remove boot.py:
   ```python
   import os
   os.remove('boot.py')
   ```
5. Press **Ctrl+X** to exit mpremote

Now `./dev.sh` should work again.

> [!TIP]
> During development, don't keep `boot.py` on the device. Only run `./upload.sh` when you need standalone operation (battery-powered, showing friends, etc.). When you're done and want to return to development, remove `boot.py` using the steps above.

## Controls

- **D-pad**: Navigate / Move character
- **A/B buttons**: Action buttons
- **Menu buttons**: Additional functions

## Contributing

It's helpful to open an issue prior to making a PR to allow discussion on the changes.

It's also helpful to keep PRs small and targeted.
