# Multi-Board Support Guide

This project now supports both **ESP32-C6** and **ESP32-C3** development boards. This guide explains how to configure and use each board.

## Quick Start

### 1. Choose Your Board

Open `src/config.py` and set the board type:

```python
# For ESP32-C6 (default)
BOARD_TYPE = "ESP32-C6"

# For ESP32-C3
BOARD_TYPE = "ESP32-C3"
```

### 2. Wire Your Board

Follow the wiring diagram for your specific board (see below).

### 3. Flash and Upload

Use the appropriate commands for your board type (see Installation section in README.md).

---

## Board Comparison

| Feature | ESP32-C6 SuperMini | ESP32-C3 |
|---------|-------------------|----------|
| CPU | RISC-V single-core | RISC-V single-core |
| GPIO Pins | GPIO0-GPIO30 | GPIO0-GPIO21 |
| I2C Pins Used | GPIO4 (SDA), GPIO7 (SCL) | GPIO6 (SDA), GPIO7 (SCL) |
| Configuration | Default | Set `BOARD_TYPE = "ESP32-C3"` |

---

## Pin Mappings

### ESP32-C6 Pin Configuration

```
Display (I2C):
├─ VCC → 3V3
├─ GND → GND
├─ SDA → GPIO4
└─ SCL → GPIO7

Buttons:
├─ UP    → GPIO14
├─ DOWN  → GPIO18
├─ LEFT  → GPIO20
├─ RIGHT → GPIO19
├─ A     → GPIO1
├─ B     → GPIO0
└─ MENU  → GPIO3
```

**Why these pins?**
- Uses higher GPIO numbers available on ESP32-C6
- Avoids commonly used pins for other peripherals
- Well-distributed across the pin layout

### ESP32-C3 Pin Configuration

```
Display (I2C):
├─ VCC → 3V3
├─ GND → GND
├─ SDA → GPIO6
└─ SCL → GPIO7

Buttons:
├─ UP    → GPIO0
├─ DOWN  → GPIO1
├─ LEFT  → GPIO2
├─ RIGHT → GPIO3
├─ A     → GPIO4
├─ B     → GPIO5
└─ MENU  → GPIO10
```

**Why these pins?**
- Uses lower GPIO pins (GPIO0-GPIO10)
- **Avoids strapping pins** GPIO8 and GPIO9 (prevents boot issues)
- Widely available on ESP32-C3 development boards
- I2C on GPIO6/7 is a common configuration

---

## Strapping Pins Warning

### ESP32-C3 Strapping Pins to AVOID:
- **GPIO2**: Boot mode selection
- **GPIO8**: Boot mode, chip mode
- **GPIO9**: Boot mode

These pins are checked during boot and can prevent the device from starting properly if connected to buttons or other peripherals.

**Our configuration avoids these pins completely.**

---

## Testing Your Hardware

Run the hardware test script to verify your wiring:

```bash
./test_hardware.sh
```

The script will:
1. Auto-detect your board type from `config.py`
2. Test the I2C display with correct pins
3. Test all buttons with correct pin assignments

---

## Switching Boards

If you want to switch from one board to another:

1. **Update configuration:**
   ```python
   # In src/config.py
   BOARD_TYPE = "ESP32-C3"  # or "ESP32-C6"
   ```

2. **Re-upload the code:**
   ```bash
   ./upload.sh
   ```

3. **Test the hardware:**
   ```bash
   ./test_hardware.sh
   ```

That's it! The code automatically uses the correct pins for your board.

---

## Troubleshooting

### Display not detected

**ESP32-C6:**
- Check GPIO4 (SDA) and GPIO7 (SCL) connections
- Verify 3V3 and GND are connected

**ESP32-C3:**
- Check GPIO6 (SDA) and GPIO7 (SCL) connections
- Verify 3V3 and GND are connected

### Buttons not working

1. Verify `BOARD_TYPE` in `config.py` matches your physical board
2. Run `./test_hardware.sh` to see which buttons are detected
3. Check button wiring matches the pin diagram for your board
4. Ensure buttons connect GPIO to GND (not to VCC)

### Device won't boot (ESP32-C3)

- Make sure you're not using GPIO2, GPIO8, or GPIO9 for buttons
- Our default configuration avoids these pins
- If you modified the pin assignments, revert to defaults

### Wrong board type error

If you see an error like:
```
ValueError: Unknown BOARD_TYPE: ESP32-XX
```

- Check the `BOARD_TYPE` variable in `src/config.py`
- Valid values are: `"ESP32-C6"` or `"ESP32-C3"` (case-sensitive)

---

## Technical Details

### How It Works

The `config.py` file contains two pin configuration dictionaries:

```python
_ESP32_C6_CONFIG = {
    'I2C_SDA': 4,
    'I2C_SCL': 7,
    'BTN_UP': 14,
    # ... etc
}

_ESP32_C3_CONFIG = {
    'I2C_SDA': 6,
    'I2C_SCL': 7,
    'BTN_UP': 0,
    # ... etc
}
```

At runtime, the appropriate configuration is selected:

```python
if BOARD_TYPE == "ESP32-C3":
    _CONFIG = _ESP32_C3_CONFIG
elif BOARD_TYPE == "ESP32-C6":
    _CONFIG = _ESP32_C6_CONFIG
```

All pin assignments are then pulled from the selected configuration:

```python
I2C_SDA = _CONFIG['I2C_SDA']  # Will be 4 or 6 depending on board
BTN_UP = _CONFIG['BTN_UP']     # Will be 14 or 0 depending on board
```

This means:
- ✅ No changes needed to any other code files
- ✅ One simple change to switch boards
- ✅ Type-safe configuration (error if invalid board type)
- ✅ Self-documenting (pin mappings visible in one place)

---

## Adding Support for Other Boards

Want to add support for another ESP32 variant? Here's how:

1. **Research the pin layout** for your board
2. **Identify available GPIO pins** and avoid strapping pins
3. **Add a new configuration dictionary** in `config.py`:
   ```python
   _ESP32_S3_CONFIG = {
       'I2C_SDA': X,
       'I2C_SCL': Y,
       'BTN_UP': Z,
       # ... etc
   }
   ```
4. **Add to the selection logic**:
   ```python
   elif BOARD_TYPE == "ESP32-S3":
       _CONFIG = _ESP32_S3_CONFIG
   ```
5. **Update this guide** with the new board's wiring diagram
6. **Test thoroughly** before committing

---

## Resources

- [ESP32-C6 Datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-c6_datasheet_en.pdf)
- [ESP32-C3 Datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf)
- [MicroPython ESP32 Documentation](https://docs.micropython.org/en/latest/esp32/quickref.html)
- [GPIO Pin Strapping](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/gpio.html)
