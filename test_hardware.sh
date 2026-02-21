#!/bin/bash
# test_hardware.sh - Quick hardware test script

echo "=== Virtual Pet Hardware Test ==="
echo ""

if ! command -v mpremote &> /dev/null; then
    echo "Error: mpremote not found"
    exit 1
fi

echo "Resetting device..."
mpremote reset

echo "Waiting for device to restart..."
sleep 2

echo ""
echo "Detecting board configuration..."
echo ""
echo "Reading BOARD_TYPE from config.py..."

# Read board type from config.py
BOARD_TYPE=$(mpremote exec "
import sys
sys.path.insert(0, '.')
try:
    import config
    print(config.BOARD_TYPE)
except Exception as e:
    print('ESP32-C6')
")

echo "Board type: $BOARD_TYPE"
echo ""

echo "Test 1: Checking I2C Display..."

# Set I2C pins based on board type
if [[ "$BOARD_TYPE" == *"ESP32-C3"* ]]; then
    I2C_SDA=6
    I2C_SCL=7
    echo "Using ESP32-C3 I2C pins: SDA=GPIO6, SCL=GPIO7"
else
    I2C_SDA=4
    I2C_SCL=7
    echo "Using ESP32-C6 I2C pins: SDA=GPIO4, SCL=GPIO7"
fi

mpremote exec "from machine import Pin, I2C; i2c=I2C(0,scl=Pin($I2C_SCL),sda=Pin($I2C_SDA)); addrs=i2c.scan(); print('Display found!' if 60 in addrs else 'Display NOT found'); print('I2C addresses:', [hex(a) for a in addrs])"

echo ""
echo "Test 2: Checking Buttons..."
echo "Press buttons to test (Ctrl+C to stop)..."

# Set button pins based on board type
if [[ "$BOARD_TYPE" == *"ESP32-C3"* ]]; then
    mpremote exec "
from machine import Pin
import time

buttons = {
    'UP': Pin(0, Pin.IN, Pin.PULL_UP),
    'DOWN': Pin(1, Pin.IN, Pin.PULL_UP),
    'LEFT': Pin(2, Pin.IN, Pin.PULL_UP),
    'RIGHT': Pin(3, Pin.IN, Pin.PULL_UP),
    'A': Pin(4, Pin.IN, Pin.PULL_UP),
    'B': Pin(5, Pin.IN, Pin.PULL_UP),
    'MENU': Pin(10, Pin.IN, Pin.PULL_UP)
}

print('Press buttons... (Ctrl+C to stop)')
last_state = {name: 1 for name in buttons}

try:
    while True:
        for name, pin in buttons.items():
            val = pin.value()
            if val == 0 and last_state[name] == 1:
                print(name, 'pressed')
            last_state[name] = val
        time.sleep(0.05)
except KeyboardInterrupt:
    print('Test complete')
"
else
    mpremote exec "
from machine import Pin
import time

buttons = {
    'UP': Pin(14, Pin.IN, Pin.PULL_UP),
    'DOWN': Pin(18, Pin.IN, Pin.PULL_UP),
    'LEFT': Pin(20, Pin.IN, Pin.PULL_UP),
    'RIGHT': Pin(19, Pin.IN, Pin.PULL_UP),
    'A': Pin(1, Pin.IN, Pin.PULL_UP),
    'B': Pin(0, Pin.IN, Pin.PULL_UP),
    'MENU': Pin(3, Pin.IN, Pin.PULL_UP)
}

print('Press buttons... (Ctrl+C to stop)')
last_state = {name: 1 for name in buttons}

try:
    while True:
        for name, pin in buttons.items():
            val = pin.value()
            if val == 0 and last_state[name] == 1:
                print(name, 'pressed')
            last_state[name] = val
        time.sleep(0.05)
except KeyboardInterrupt:
    print('Test complete')
"
fi

echo ""
echo "=== Hardware Test Complete ==="
