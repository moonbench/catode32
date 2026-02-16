"""
transitions.py - Screen transition effects and management
"""

import math
import config


class TransitionManager:
    """Manages screen transition state and animation.

    Handles the two-phase transition: out (closing) then in (opening).
    Calls a callback at the midpoint (between phases) for scene switching.
    """

    def __init__(self, renderer, transition_type='fade', duration=0.3):
        """Initialize the transition manager.

        Args:
            renderer: The Renderer instance for drawing transitions
            transition_type: 'fade', 'wipe', or 'iris'
            duration: Duration of each phase in seconds
        """
        self.transition_renderer = TransitionRenderer(renderer)
        self.transition_type = transition_type
        self.duration = duration

        self.active = False
        self.progress = 0.0
        self.phase = 'out'  # 'out' (closing) or 'in' (opening)
        self._midpoint_callback = None
        self._midpoint_called = False

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

        # Cap dt to prevent jumps after slow frames
        dt = min(dt, self.duration * 0.5)

        # Advance progress
        self.progress += dt / self.duration

        if self.progress >= 1.0:
            self.progress = 1.0

            if self.phase == 'out':
                # Transition-out complete, call midpoint callback and start in phase
                if self._midpoint_callback and not self._midpoint_called:
                    self._midpoint_called = True
                    self._midpoint_callback()
                self.phase = 'in'
                self.progress = 0.0
            else:
                # Transition-in complete, end transition
                self.active = False
                self.progress = 0.0

        return True

    def draw(self):
        """Draw the transition overlay if active."""
        if not self.active:
            return

        # Calculate effective progress (inverted for 'in' phase)
        if self.phase == 'out':
            progress = self.progress
        else:
            progress = 1.0 - self.progress

        # Determine direction for wipe transitions
        direction = None
        if self.transition_type == 'wipe':
            direction = 'right' if self.phase == 'out' else 'left'

        self.transition_renderer.draw(self.transition_type, progress, direction)


class TransitionRenderer:
    """Handles drawing screen transition effects."""

    def __init__(self, renderer):
        """Initialize with a renderer instance.

        Args:
            renderer: The Renderer instance to draw with
        """
        self.renderer = renderer

    def draw_fade(self, progress):
        """Draw dither pattern overlay for fade transition.

        Args:
            progress: 0.0 = fully clear, 1.0 = fully black
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.renderer.display.fill(0)
            return

        # Use different dither patterns based on progress
        # Pattern density increases with progress
        for y in range(config.DISPLAY_HEIGHT):
            for x in range(config.DISPLAY_WIDTH):
                draw_pixel = False

                if progress < 0.25:
                    # Sparse: every 4th pixel in a grid pattern
                    threshold = progress / 0.25
                    draw_pixel = (x % 4 == 0 and y % 4 == 0) and ((x + y) % 8 < threshold * 8)
                elif progress < 0.5:
                    # Quarter fill: 2x2 grid, one pixel per cell
                    threshold = (progress - 0.25) / 0.25
                    base = (x % 2 == 0 and y % 2 == 0)
                    extra = (x % 2 == 1 and y % 2 == 1) and ((x + y) % 4 < threshold * 4)
                    draw_pixel = base or extra
                elif progress < 0.75:
                    # Checkerboard: half the pixels
                    threshold = (progress - 0.5) / 0.25
                    base = (x + y) % 2 == 0
                    extra = (x % 2 == 0 and y % 2 == 1) and ((x + y) % 4 < threshold * 4)
                    draw_pixel = base or extra
                else:
                    # Dense: three-quarters to full
                    threshold = (progress - 0.75) / 0.25
                    # Start with 3/4 filled, progress to full
                    skip = (x + y) % 2 == 1 and (x % 2 == 0) and ((x + y) % 4 >= threshold * 4)
                    draw_pixel = not skip

                if draw_pixel:
                    self.renderer.draw_pixel(x, y, 0)

    def draw_wipe(self, progress, direction='right'):
        """Draw wipe transition.

        Args:
            progress: 0.0 = no wipe, 1.0 = fully wiped
            direction: 'left', 'right', 'up', 'down'
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.renderer.display.fill(0)
            return

        if direction == 'right':
            width = int(progress * config.DISPLAY_WIDTH)
            self.renderer.draw_rect(0, 0, width, config.DISPLAY_HEIGHT, filled=True, color=0)
        elif direction == 'left':
            width = int(progress * config.DISPLAY_WIDTH)
            x = config.DISPLAY_WIDTH - width
            self.renderer.draw_rect(x, 0, width, config.DISPLAY_HEIGHT, filled=True, color=0)
        elif direction == 'down':
            height = int(progress * config.DISPLAY_HEIGHT)
            self.renderer.draw_rect(0, 0, config.DISPLAY_WIDTH, height, filled=True, color=0)
        elif direction == 'up':
            height = int(progress * config.DISPLAY_HEIGHT)
            y = config.DISPLAY_HEIGHT - height
            self.renderer.draw_rect(0, y, config.DISPLAY_WIDTH, height, filled=True, color=0)

    def draw_iris(self, progress):
        """Draw iris (circle) transition.

        Args:
            progress: 0.0 = fully open (no black), 1.0 = fully closed (all black)
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.renderer.display.fill(0)
            return

        # Center of screen
        cx = config.DISPLAY_WIDTH // 2
        cy = config.DISPLAY_HEIGHT // 2

        # Max radius is distance from center to corner
        max_radius = int(math.sqrt(cx * cx + cy * cy)) + 1

        # Current hole radius (shrinks as progress increases)
        hole_radius = int(max_radius * (1 - progress))
        hole_radius_sq = hole_radius * hole_radius

        # Draw black pixels outside the circle
        for y in range(config.DISPLAY_HEIGHT):
            dy = y - cy
            dy_sq = dy * dy
            for x in range(config.DISPLAY_WIDTH):
                dx = x - cx
                dist_sq = dx * dx + dy_sq
                if dist_sq > hole_radius_sq:
                    self.renderer.draw_pixel(x, y, 0)

    def draw(self, transition_type, progress, direction=None):
        """Draw the appropriate transition effect.

        Args:
            transition_type: 'fade', 'wipe', or 'iris'
            progress: 0.0 to 1.0
            direction: direction for wipe transition (optional)
        """
        if transition_type == 'fade':
            self.draw_fade(progress)
        elif transition_type == 'wipe':
            self.draw_wipe(progress, direction or 'right')
        elif transition_type == 'iris':
            self.draw_iris(progress)
