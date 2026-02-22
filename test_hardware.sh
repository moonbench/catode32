#!/bin/bash
# test_hardware.sh - Quick hardware test script
# Reads all pin configurations directly from config.py
RED="\e[1;31m"
GREEN="\e[1;32m"
YELLOW="\e[1;33m"
BLUE="\e[1;34m"
MAGENTA="\e[1;35m"
CYAN="\e[1;36m"
RESET="\e[0m"

echo "=== Virtual Pet Hardware Test ==="
echo ""

if ! command -v mpremote &> /dev/null; then
    echo "Error: mpremote not found"
    exit 1
fi


echo "Resetting device..."
mpremote reset

sleep 2

echo -e "Device restarted. Checking connection..."
if ! mpremote ls &> /dev/null; then
    echo -e "${RED}Error: Unable to connect to device after reset${RESET} ⚠️"
    exit 1
else
    echo -e "${GREEN}Device connected successfully! ✅${RESET}"
fi

echo -e "\nUploading config.py to device..."
if mpremote ls | grep -q 'config.py'; then
    mpremote rm config.py
fi
mpremote cp src/config.py :/config.py

echo -e "\nReading configuration from config.py..."
# Read all configuration from config.py in one call
CONFIG=$(mpremote exec "
import sys
sys.path.insert(0, '.')
try:
    import config
    print(f'BOARD_TYPE={config.BOARD_TYPE}')
    print(f'I2C_SDA={config.I2C_SDA}')
    print(f'I2C_SCL={config.I2C_SCL}')
    print(f'BTN_UP={config.BTN_UP}')
    print(f'BTN_DOWN={config.BTN_DOWN}')
    print(f'BTN_LEFT={config.BTN_LEFT}')
    print(f'BTN_RIGHT={config.BTN_RIGHT}')
    print(f'BTN_A={config.BTN_A}')
    print(f'BTN_B={config.BTN_B}')
    print(f'BTN_MENU={config.BTN_MENU1}')
    print(f'HOLD_TIME_MS={config.BUTTON_HOLD_TIME_MS}')
except Exception:
    # Default values
    print(f'ERROR_READING_CONFIG=True')
")

# Evaluate the assignments in the current shell
eval "$(echo "$CONFIG" | tr -d '\r')"

if [[ "$ERROR_READING_CONFIG" == "True" ]]; then
    echo -e "${RED}Error: Unsupported BOARD_TYPE${RESET} ⚠️"
    echo -e "${YELLOW}Please set valid BOARD_TYPE in config.py and try again.${RESET}"
    echo -e "Currently supported boards: ${CYAN}ESP32-C6${RESET}, ${CYAN}ESP32-C3${RESET}"
    exit 1
fi

echo -e "${YELLOW}Configuration read from device:${RESET}"
echo "=============================="

echo -e "Board type: ${CYAN}$BOARD_TYPE${RESET} ✅"
echo -e "I2C pins:"
echo -e "  SDA=${CYAN}GPIO_$I2C_SDA${RESET}"
echo -e "  SCL=${CYAN}GPIO_$I2C_SCL${RESET}"
echo -e "Navigation Buttons:"
echo -e "  UP=${MAGENTA}GPIO_$BTN_UP${RESET}"
echo -e "  DOWN=${MAGENTA}GPIO_$BTN_DOWN${RESET}"
echo -e "  LEFT=${MAGENTA}GPIO_$BTN_LEFT${RESET}"
echo -e "  RIGHT=${MAGENTA}GPIO_$BTN_RIGHT${RESET}"
echo -e "Function Buttons:"
echo -e "  A=${BLUE}GPIO_$BTN_A${RESET}"
echo -e "  B=${BLUE}GPIO_$BTN_B${RESET}"
echo -e "  MENU=${BLUE}GPIO_$BTN_MENU${RESET}"
echo -e "Long press Button threshold: ${GREEN}${HOLD_TIME_MS}ms${RESET}"
echo ""

echo "Test 1: Checking I2C Display..."
# mpremote exec "from machine import Pin, I2C; i2c=I2C(0,scl=Pin($I2C_SCL),sda=Pin($I2C_SDA)); addrs=i2c.scan(); print('Display found!' if 60 in addrs else 'Display NOT found'); print('I2C addresses:', [hex(a) for a in addrs])"
mpremote exec "from machine import Pin, I2C; \
i2c=I2C(0,scl=Pin($I2C_SCL),sda=Pin($I2C_SDA)); \
addrs=i2c.scan(); \
print('\033[1;32mDisplay found!✅\033[0m' if 60 in addrs else '\033[1;31mDisplay NOT found ❌\033[0m'); \
print('\033[1;36mI2C addresses:\033[0m', [hex(a) for a in addrs])"

echo ""
echo "Test 2: Checking Buttons (with long press detection)..."

# Use the pin values read from config.py
mpremote exec """
from machine import Pin
import time

buttons = {
    'UP': Pin($BTN_UP, Pin.IN, Pin.PULL_UP),
    'DOWN': Pin($BTN_DOWN, Pin.IN, Pin.PULL_UP),
    'LEFT': Pin($BTN_LEFT, Pin.IN, Pin.PULL_UP),
    'RIGHT': Pin($BTN_RIGHT, Pin.IN, Pin.PULL_UP),
    'A': Pin($BTN_A, Pin.IN, Pin.PULL_UP),
    'B': Pin($BTN_B, Pin.IN, Pin.PULL_UP),
    'MENU': Pin($BTN_MENU, Pin.IN, Pin.PULL_UP)
}

# Long press threshold in milliseconds
HOLD_TIME_MS = $HOLD_TIME_MS

print('Press buttons to test (Ctrl+C to stop)...')
last_state = {name: 1 for name in buttons}
press_start_time = {name: 0 for name in buttons}
long_press_triggered = {name: False for name in buttons}

try:
    while True:
        current_time = time.ticks_ms()
        
        for name, pin in buttons.items():
            val = pin.value()
            
            # Button just pressed (transition from 1 to 0)
            if val == 0 and last_state[name] == 1:
                print(f'\n\033[1;32m{name}\033[0m pressed')
                press_start_time[name] = current_time
                long_press_triggered[name] = False
            
            # Button being held
            elif val == 0 and last_state[name] == 0:
                if not long_press_triggered[name]:
                    hold_duration = time.ticks_diff(current_time, press_start_time[name])
                    if hold_duration >= HOLD_TIME_MS:
                        print(f'\033[1;35m___{name}\033[0m LONG PRESSED (held for {hold_duration}ms)')
                        long_press_triggered[name] = True
            
            # Button released
            elif val == 1 and last_state[name] == 0:
                hold_duration = time.ticks_diff(current_time, press_start_time[name])
                if not long_press_triggered[name]:
                    print(f'\033[1;32m_{name}\033[0m released (held for {hold_duration}ms)')
                else:
                    print(f'\033[1;35m___{name}\033[0m released after long press')
                long_press_triggered[name] = False
            
            last_state[name] = val
        
        time.sleep(0.05)
except KeyboardInterrupt:
    print('Test complete')
"""

echo ""
echo "=== Hardware Test Complete ==="
