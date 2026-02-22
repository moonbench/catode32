"""
input.py - Button input handling with debouncing
"""

from machine import Pin
import time
import config

class InputHandler:
    """Handles button inputs with debouncing and state tracking"""
    
    def __init__(self):
        # Initialize all buttons with internal pull-ups
        self.buttons = {
            'up': Pin(config.BTN_UP, Pin.IN, Pin.PULL_UP),
            'down': Pin(config.BTN_DOWN, Pin.IN, Pin.PULL_UP),
            'left': Pin(config.BTN_LEFT, Pin.IN, Pin.PULL_UP),
            'right': Pin(config.BTN_RIGHT, Pin.IN, Pin.PULL_UP),
            'a': Pin(config.BTN_A, Pin.IN, Pin.PULL_UP),
            'b': Pin(config.BTN_B, Pin.IN, Pin.PULL_UP),
            'menu1': Pin(config.BTN_MENU1, Pin.IN, Pin.PULL_UP)
        }
        
        # Track button states for debouncing
        self.button_states = {}
        self.last_press_time = {}
        self.debounce_time_ms = 50  # 50ms debounce
        self.hold_time_ms = config.BUTTON_HOLD_TIME_MS  # Configurable long press detection
        
        # Track if long press has been triggered
        self.long_press_triggered = {}
        
        # Initialize state tracking
        for btn_name in self.buttons:
            self.button_states[btn_name] = False
            self.last_press_time[btn_name] = 0
            self.long_press_triggered[btn_name] = False
    
    def is_pressed(self, button_name):
        """
        Check if a button is currently pressed (raw state, no debouncing)
        Returns True if pressed, False otherwise
        """
        if button_name not in self.buttons:
            return False
        # Button is active low (0 = pressed)
        return self.buttons[button_name].value() == 0
    
    def was_just_pressed(self, button_name):
        """
        Check if a button was just pressed (with debouncing)
        Returns True only on the rising edge of a button press (short press)
        Does not trigger if the button is held long enough for a long press
        """
        if button_name not in self.buttons:
            return False
        
        current_time = time.ticks_ms()
        is_currently_pressed = self.is_pressed(button_name)
        was_previously_pressed = self.button_states[button_name]
        time_since_last = time.ticks_diff(current_time, self.last_press_time[button_name])
        
        # Button just pressed (wasn't pressed before, is pressed now)
        if is_currently_pressed and not was_previously_pressed:
            # Check debounce time
            if time_since_last > self.debounce_time_ms:
                self.button_states[button_name] = True
                self.last_press_time[button_name] = current_time
                self.long_press_triggered[button_name] = False
                return True
        
        # Button released - trigger short press only if long press didn't trigger
        if not is_currently_pressed and was_previously_pressed:
            self.button_states[button_name] = False
            # Reset long press flag for next press
            self.long_press_triggered[button_name] = False
        
        return False
    
    def was_long_pressed(self, button_name):
        """
        Check if a button was held down for long press duration
        Returns True once when the hold threshold is reached
        """
        if button_name not in self.buttons:
            return False
        
        current_time = time.ticks_ms()
        is_currently_pressed = self.is_pressed(button_name)
        was_previously_pressed = self.button_states[button_name]
        time_since_last = time.ticks_diff(current_time, self.last_press_time[button_name])
        
        # Button just pressed (initialize state)
        if is_currently_pressed and not was_previously_pressed:
            # Check debounce time
            if time_since_last > self.debounce_time_ms:
                self.button_states[button_name] = True
                self.last_press_time[button_name] = current_time
                self.long_press_triggered[button_name] = False
        
        # Button is being held - check for long press
        elif is_currently_pressed and was_previously_pressed:
            # Check if hold time reached and not already triggered
            if not self.long_press_triggered[button_name]:
                time_held = time.ticks_diff(current_time, self.last_press_time[button_name])
                if time_held >= self.hold_time_ms:
                    self.long_press_triggered[button_name] = True
                    return True
        
        # Button released - reset state
        elif not is_currently_pressed and was_previously_pressed:
            self.button_states[button_name] = False
            self.long_press_triggered[button_name] = False
        
        return False
    
    def was_released_after_hold(self, button_name):
        """
        Check if a button was just released and return the hold duration
        Returns the hold time in ms if button was just released, or -1 if not
        This is useful for distinguishing short press from long press on release
        """
        if button_name not in self.buttons:
            return -1
        
        current_time = time.ticks_ms()
        is_currently_pressed = self.is_pressed(button_name)
        was_previously_pressed = self.button_states[button_name]
        
        # Button just pressed (initialize state)
        if is_currently_pressed and not was_previously_pressed:
            time_since_last = time.ticks_diff(current_time, self.last_press_time[button_name])
            if time_since_last > self.debounce_time_ms:
                self.button_states[button_name] = True
                self.last_press_time[button_name] = current_time
                self.long_press_triggered[button_name] = False
        
        # Button released - return hold duration
        elif not is_currently_pressed and was_previously_pressed:
            hold_time = time.ticks_diff(current_time, self.last_press_time[button_name])
            self.button_states[button_name] = False
            self.long_press_triggered[button_name] = False
            return hold_time
        
        return -1
    
    def get_direction(self):
        """
        Get the current direction from D-pad buttons
        Returns tuple (dx, dy) for movement delta
        """
        dx = 0
        dy = 0
        
        if self.is_pressed('up'):
            dy -= 1
        if self.is_pressed('down'):
            dy += 1
        if self.is_pressed('left'):
            dx -= 1
        if self.is_pressed('right'):
            dx += 1
        
        return (dx, dy)
    
    def any_button_pressed(self):
        """Check if any button is currently pressed"""
        return any(self.is_pressed(btn) for btn in self.buttons)
    
    def get_pressed_buttons(self):
        """Get list of all currently pressed button names"""
        return [name for name in self.buttons if self.is_pressed(name)]