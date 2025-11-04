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
echo "Test 1: Checking I2C Display..."
mpremote exec "from machine import Pin, I2C; i2c=I2C(0,scl=Pin(7),sda=Pin(4)); addrs=i2c.scan(); print('Display found!' if 60 in addrs else 'Display NOT found'); print('I2C addresses:', [hex(a) for a in addrs])"

echo ""
echo "Test 2: Checking Buttons..."
echo "Press buttons to test (Ctrl+C to stop)..."
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
    'MENU1': Pin(3, Pin.IN, Pin.PULL_UP),
    'MENU2': Pin(2, Pin.IN, Pin.PULL_UP)
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
