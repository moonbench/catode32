#!/bin/bash
# test_hardware.sh - Quick hardware test script
# Reads all pin configurations directly from config.py

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
echo "Reading configuration from config.py..."

# Read all configuration from config.py in one call
CONFIG=$(mpremote exec "
import sys
sys.path.insert(0, '.')
try:
    import config
    # Print all needed values on separate lines
    print(config.BOARD_TYPE)
    print(config.I2C_SDA)
    print(config.I2C_SCL)
    print(config.BTN_UP)
    print(config.BTN_DOWN)
    print(config.BTN_LEFT)
    print(config.BTN_RIGHT)
    print(config.BTN_A)
    print(config.BTN_B)
    print(config.BTN_MENU1)
    print(config.BTN_MENU2)
except Exception as e:
    # Default to ESP32-C6 values if config can't be read
    print('ESP32-C6')
    print('4')
    print('7')
    print('14')
    print('18')
    print('20')
    print('19')
    print('1')
    print('0')
    print('3')
    print('2')
")

# Parse the configuration values
BOARD_TYPE=$(echo "$CONFIG" | sed -n '1p')
I2C_SDA=$(echo "$CONFIG" | sed -n '2p')
I2C_SCL=$(echo "$CONFIG" | sed -n '3p')
BTN_UP=$(echo "$CONFIG" | sed -n '4p')
BTN_DOWN=$(echo "$CONFIG" | sed -n '5p')
BTN_LEFT=$(echo "$CONFIG" | sed -n '6p')
BTN_RIGHT=$(echo "$CONFIG" | sed -n '7p')
BTN_A=$(echo "$CONFIG" | sed -n '8p')
BTN_B=$(echo "$CONFIG" | sed -n '9p')
BTN_MENU1=$(echo "$CONFIG" | sed -n '10p')
BTN_MENU2=$(echo "$CONFIG" | sed -n '11p')

echo "Board type: $BOARD_TYPE"
echo "I2C pins: SDA=GPIO$I2C_SDA, SCL=GPIO$I2C_SCL"
echo "Button pins: UP=$BTN_UP, DOWN=$BTN_DOWN, LEFT=$BTN_LEFT, RIGHT=$BTN_RIGHT, A=$BTN_A, B=$BTN_B, MENU1=$BTN_MENU1, MENU2=$BTN_MENU2"
echo ""

echo "Test 1: Checking I2C Display..."
mpremote exec "from machine import Pin, I2C; i2c=I2C(0,scl=Pin($I2C_SCL),sda=Pin($I2C_SDA)); addrs=i2c.scan(); print('Display found!' if 60 in addrs else 'Display NOT found'); print('I2C addresses:', [hex(a) for a in addrs])"

echo ""
echo "Test 2: Checking Buttons..."
echo "Press buttons to test (Ctrl+C to stop)..."

# Use the pin values read from config.py
mpremote exec "
from machine import Pin
import time

buttons = {
    'UP': Pin($BTN_UP, Pin.IN, Pin.PULL_UP),
    'DOWN': Pin($BTN_DOWN, Pin.IN, Pin.PULL_UP),
    'LEFT': Pin($BTN_LEFT, Pin.IN, Pin.PULL_UP),
    'RIGHT': Pin($BTN_RIGHT, Pin.IN, Pin.PULL_UP),
    'A': Pin($BTN_A, Pin.IN, Pin.PULL_UP),
    'B': Pin($BTN_B, Pin.IN, Pin.PULL_UP),
    'MENU1': Pin($BTN_MENU1, Pin.IN, Pin.PULL_UP),
    'MENU2': Pin($BTN_MENU2, Pin.IN, Pin.PULL_UP)
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

echo ""
echo "=== Hardware Test Complete ==="
