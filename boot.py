# boot.py - Auto-run game unless A+B buttons held during boot
#
# Hold A+B when powering on or pressing reset to skip auto-run
# and get a REPL for development/debugging with mpremote.
#
# There's also a 1-second delay before auto-run to allow mpremote
# to interrupt via Ctrl+C.

from machine import Pin
import time

# Button pins (from config.py)
BTN_A = 1
BTN_B = 0

# Set up buttons with pull-up (pressed = low)
btn_a = Pin(BTN_A, Pin.IN, Pin.PULL_UP)
btn_b = Pin(BTN_B, Pin.IN, Pin.PULL_UP)

# Small delay to let buttons settle
time.sleep_ms(100)

# Check if A+B held (both low = pressed)
if btn_a.value() == 0 and btn_b.value() == 0:
    print("A+B held - skipping auto-run (REPL mode)")
else:
    # Delay to allow mpremote/Ctrl+C to interrupt
    print("Starting in 1s... (Ctrl+C to cancel)")
    time.sleep(1)
    print("Starting game...")
    try:
        import main
        main.main()
    except Exception as e:
        import sys
        print("Error starting game:")
        sys.print_exception(e)
