"""
transitions.py - Screen transition effects and management
"""

import config


class TransitionManager:
    """Manages screen transition state and animation.

    Handles the two-phase transition: out (closing) then in (opening).
    Calls a callback at the midpoint (between phases) for scene switching.
    """

    def __init__(self, renderer, duration=0.3):
        """Initialize the transition manager.

        Args:
            renderer: The Renderer instance for drawing transitions
            duration: Duration of each phase in seconds
        """
        self.display = renderer.display
        self.duration = duration

        self.active = False
        self.progress = 0.0
        self.phase = 'out'  # 'out' (closing) or 'in' (opening)
        self._midpoint_callback = None
        self._midpoint_called = False
        self._ready_for_midpoint = False  # True after one fully-black frame, before callback fires

    def start(self, on_midpoint=None):
        """Start a transition.

        Args:
            on_midpoint: Callback function to call at the transition midpoint
                         (when screen is fully covered, before opening)
        """
        if self.active:
            return False

        self.active = True
        self.phase = 'out'
        self.progress = 0.0
        self._midpoint_callback = on_midpoint
        self._midpoint_called = False
        self._ready_for_midpoint = False
        return True

    def update(self, dt):
        """Update transition animation.

        Args:
            dt: Delta time since last update

        Returns:
            True if transition is active and consumed the update
        """
        if not self.active:
            return False

        # Fire the midpoint callback after one fully-black frame has been shown.
        # This avoids freezing on the near-complete dither while the scene loads.
        if self._ready_for_midpoint:
            self._ready_for_midpoint = False
            if self._midpoint_callback and not self._midpoint_called:
                self._midpoint_called = True
                self._midpoint_callback()
            self.phase = 'in'
            self.progress = 0.0
            return True

        # Cap dt to one nominal frame so the animation plays at the right speed
        # even after a slow scene load.
        dt = min(dt, 1.0 / config.FPS)

        # Advance progress
        self.progress += dt / self.duration

        if self.progress >= 1.0:
            self.progress = 1.0

            if self.phase == 'out':
                # Hold at full black for one frame so show() runs before the callback.
                self._ready_for_midpoint = True
            else:
                # Transition-in complete, end transition
                self.active = False
                self.progress = 0.0

        return True

    def draw(self):
        """Draw the transition overlay if active."""
        if not self.active:
            return

        progress = self.progress if self.phase == 'out' else 1.0 - self.progress

        if progress <= 0:
            return
        if progress >= 1:
            self.display.fill(0)
            return

        # 8-pass scanline interlace: each pass fills every 8th row at a
        # different offset (0..7), adding ~12.5 % coverage per pass.
        passes = int(progress * 8) + 1
        if passes >= 8:
            self.display.fill(0)
        else:
            w = config.DISPLAY_WIDTH
            h = config.DISPLAY_HEIGHT
            for offset in range(passes):
                for y in range(offset, h, 8):
                    self.display.hline(0, y, w, 0)
