"""gardening_ui.py - Interactive UI modes for the plant system.

PlacementMode handles the cursor-based pot/seed placement flow that replaces
normal scene panning when the player is placing a new pot.

PlantSelectionMode lets the player cycle through existing plants in a scene
to select one for watering, tending, or seeding into an empty pot.
"""

import config
from plant_system import place_empty_pot
from assets.plants import POT_SPRITES, PLANT_SPRITES
from assets.icons import PLACE_DOWN_ICON
from environment import PARALLAX_FACTORS

_DEFAULT_SURFACE = {'y_snap': 63, 'layer': 'foreground'}

_STEP = 8   # px per d-pad press

# Secondary sort key for surfaces with the same y_snap.
# Foreground sorts last so start_idx always lands on the fg floor.
_LAYER_ORDER = {'background': 0, 'midground': 1, 'foreground': 2}


class PlacementMode:
    """Cursor mode for placing a pot in a scene.

    Usage:
        mode = PlacementMode()
        mode.enter(pot_type, scene)          # activate from menu action
        mode.handle_input(input, environment) # call from handle_input
        mode.draw(renderer, environment)      # call from draw
    """

    # Bounce period in seconds: icon alternates between two positions.
    _BOUNCE_PERIOD = 0.4

    def __init__(self):
        self.active = False
        self._pot_type    = None
        self._surfaces    = []    # sorted surface list (y_snap ascending)
        self._surface_idx = 0
        self._cursor_x    = 0
        self._scene       = None  # weak ref to host scene (not persisted)
        self._bounce_t    = 0.0   # accumulator for bounce animation
        self._on_confirm  = None  # optional callable(layer, x, y_snap); overrides default place_empty_pot

    # ------------------------------------------------------------------
    # Activation
    # ------------------------------------------------------------------

    def enter(self, pot_type, scene, on_confirm=None):
        """Activate placement mode for the given pot type in the given scene.

        on_confirm: optional callable(layer, x, y_snap) called instead of
        place_empty_pot when the player confirms.  Use this when repositioning
        an existing plant rather than placing a new pot.
        """
        raw = list(getattr(scene, 'PLANT_SURFACES', None) or [_DEFAULT_SURFACE])
        # Primary sort: y_snap ascending (higher on screen first).
        # Secondary sort: layer order ascending so foreground is always last —
        # start_idx lands on the fg floor, and up/down feel natural.
        surfaces = sorted(raw, key=lambda s: (s['y_snap'], _LAYER_ORDER.get(s['layer'], 1)))
        start_idx = len(surfaces) - 1   # start on foreground floor
        surf = surfaces[start_idx]
        x_min = surf.get('x_min', 0)
        x_max = surf.get('x_max', self._surface_x_max(surf, scene.environment.world_width))
        cursor_x = int(scene.environment.camera_x) + config.DISPLAY_WIDTH // 2
        cursor_x = max(x_min, min(x_max, cursor_x))

        self.active        = True
        self._pot_type     = pot_type
        self._surfaces     = surfaces
        self._surface_idx  = start_idx
        self._cursor_x     = cursor_x
        self._scene        = scene
        self._on_confirm   = on_confirm

    def cancel(self):
        self.active = False
        self._bounce_t = 0.0
        self._scene = None
        self._on_confirm = None

    # ------------------------------------------------------------------
    # Input handling
    # ------------------------------------------------------------------

    def update(self, dt):
        self._bounce_t = (self._bounce_t + dt) % (self._BOUNCE_PERIOD * 2)

    def handle_input(self, input_handler, environment):
        """Process one frame of placement input.  Returns None always."""
        if input_handler.was_just_pressed('b'):
            self.cancel()
            return None

        if input_handler.was_just_pressed('a'):
            self._confirm(environment)
            return None

        # Left / Right: move cursor in fixed steps.
        dx = 0
        if input_handler.was_just_pressed('right'):
            dx = _STEP
        elif input_handler.was_just_pressed('left'):
            dx = -_STEP
        if dx:
            surf = self._surfaces[self._surface_idx]
            x_min = surf.get('x_min', 0)
            x_max = surf.get('x_max', self._surface_x_max(surf, self._scene.environment.world_width))
            self._cursor_x = max(x_min, min(x_max, self._cursor_x + dx))
            self._follow_camera(environment)

        # Up / Down: cycle between surfaces, adjusting cursor_x to keep
        # the same screen position across layers.
        surfaces = self._surfaces
        sidx = self._surface_idx
        if input_handler.was_just_pressed('up') and sidx > 0:
            self._switch_surface(sidx - 1, environment.camera_x)
        elif input_handler.was_just_pressed('down') and sidx < len(surfaces) - 1:
            self._switch_surface(sidx + 1, environment.camera_x)

        return None

    # ------------------------------------------------------------------
    # Drawing
    # ------------------------------------------------------------------

    def draw(self, renderer, environment):
        surf     = self._surfaces[self._surface_idx]
        parallax = PARALLAX_FACTORS.get(surf.get('layer', 'foreground'), 1.0)
        sx = self._cursor_x - int(environment.camera_x * parallax)
        bounce_offset = 0 if self._bounce_t < self._BOUNCE_PERIOD else 2

        pot_spr = POT_SPRITES.get(self._pot_type)
        if pot_spr is not None:
            sy = surf['y_snap'] - pot_spr['height']
            renderer.draw_sprite_obj(pot_spr, sx, sy)
            icon_x = sx + pot_spr['width'] // 2 - PLACE_DOWN_ICON['width'] // 2
        elif self._pot_type == 'ground':
            sy = surf['y_snap']
            icon_x = sx - PLACE_DOWN_ICON['width'] // 2
        else:
            return

        icon_y = sy - PLACE_DOWN_ICON['height'] - 2 + bounce_offset
        renderer.draw_sprite_obj(PLACE_DOWN_ICON, icon_x, icon_y)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _follow_camera(self, environment):
        surf     = self._surfaces[self._surface_idx]
        parallax = PARALLAX_FACTORS.get(surf.get('layer', 'foreground'), 1.0)
        margin   = 32
        sx = self._cursor_x - int(environment.camera_x * parallax)
        if sx < margin:
            environment.set_camera(int((self._cursor_x - margin) / parallax))
        elif sx > config.DISPLAY_WIDTH - margin:
            environment.set_camera(int((self._cursor_x - (config.DISPLAY_WIDTH - margin)) / parallax))

    def _surface_x_max(self, surf, world_width):
        """Compute the maximum world-x for a surface that keeps the pot on screen.

        A plant at world_x on a layer with parallax p appears at
        screen_x = world_x - camera_x * p.  At maximum camera pan the
        effective right edge in world-space is:
            DISPLAY_WIDTH * (1 - p) + world_width * p
        """
        p = PARALLAX_FACTORS.get(surf.get('layer', 'foreground'), 1.0)
        return int(config.DISPLAY_WIDTH * (1.0 - p) + world_width * p)

    def _switch_surface(self, new_idx, camera_x):
        """Switch to a new surface index, adjusting cursor_x to keep screen position."""
        old_surf = self._surfaces[self._surface_idx]
        old_p    = PARALLAX_FACTORS.get(old_surf.get('layer', 'foreground'), 1.0)
        self._surface_idx = new_idx
        new_surf = self._surfaces[new_idx]
        new_p    = PARALLAX_FACTORS.get(new_surf.get('layer', 'foreground'), 1.0)
        # Solve for new world_x that gives the same screen_x:
        # screen_x = world_x - camera_x * parallax
        self._cursor_x = int(self._cursor_x + camera_x * (new_p - old_p))
        x_min = new_surf.get('x_min', 0)
        x_max = new_surf.get('x_max', self._surface_x_max(new_surf, self._scene.environment.world_width))
        self._cursor_x = max(x_min, min(x_max, self._cursor_x))

    def _confirm(self, environment):
        scene = self._scene
        surf  = self._surfaces[self._surface_idx]
        if self._on_confirm:
            self._on_confirm(surf['layer'], self._cursor_x, surf['y_snap'])
        else:
            scene_count = sum(1 for p in scene.context.plants
                              if p['scene'] == scene.SCENE_NAME)
            if scene_count < 8:
                place_empty_pot(
                    scene.context,
                    scene.SCENE_NAME,
                    surf['layer'],
                    self._cursor_x,
                    surf['y_snap'],
                    self._pot_type,
                )
        self.cancel()


class PlantSelectionMode:
    """Cursor mode for cycling through existing plants in a scene to select one.

    Usage:
        mode = PlantSelectionMode()
        mode.enter(scene, on_confirm, filter_fn)  # activate; returns False if no plants
        mode.update(dt)                            # call from scene update
        mode.handle_input(input, environment)      # call from scene handle_input
        mode.draw(renderer, environment)           # call from scene draw

    on_confirm(plant) is called when the player presses A; on_confirm is NOT called
    on cancel (B press) — the mode simply deactivates.
    """

    _BOUNCE_PERIOD = 0.4

    def __init__(self):
        self.active = False
        self._plants = []
        self._idx = 0
        self._scene = None
        self._bounce_t = 0.0
        self._on_confirm = None

    # ------------------------------------------------------------------
    # Activation
    # ------------------------------------------------------------------

    def enter(self, scene, on_confirm, filter_fn=None):
        """Activate selection mode.

        Args:
            scene: the host MainScene instance
            on_confirm: callable(plant_dict) invoked when A is pressed
            filter_fn: optional callable(plant_dict) → bool to restrict which
                       plants are selectable (e.g. only empty pots)

        Returns True if at least one selectable plant was found, False otherwise.
        """
        plants = [p for p in scene.context.plants if p['scene'] == scene.SCENE_NAME]
        if filter_fn:
            plants = [p for p in plants if filter_fn(p)]
        if not plants:
            return False

        plants.sort(key=lambda p: p['x'])

        self.active = True
        self._plants = plants
        self._idx = 0
        self._scene = scene
        self._bounce_t = 0.0
        self._on_confirm = on_confirm
        self._follow_camera(scene.environment)
        return True

    def cancel(self):
        self.active = False
        self._scene = None
        self._on_confirm = None

    # ------------------------------------------------------------------
    # Update / input / draw
    # ------------------------------------------------------------------

    def update(self, dt):
        self._bounce_t = (self._bounce_t + dt) % (self._BOUNCE_PERIOD * 2)

    def handle_input(self, input_handler, environment):
        """Process one frame of selection input. Returns None always."""
        if input_handler.was_just_pressed('b'):
            self.cancel()
            return None

        if input_handler.was_just_pressed('a'):
            plant = self._plants[self._idx]
            cb = self._on_confirm
            self.active = False
            self._scene = None
            self._on_confirm = None
            if cb:
                cb(plant)
            return None

        n = len(self._plants)
        if n > 1:
            if input_handler.was_just_pressed('right'):
                self._idx = (self._idx + 1) % n
                self._follow_camera(environment)
            elif input_handler.was_just_pressed('left'):
                self._idx = (self._idx - 1) % n
                self._follow_camera(environment)

        return None

    def draw(self, renderer, environment):
        if not self._plants:
            return
        plant = self._plants[self._idx]
        layer = plant.get('layer', 'foreground')
        parallax = PARALLAX_FACTORS.get(layer, 1.0)

        pot_type = plant.get('pot', 'small')
        if pot_type != 'ground':
            pot_spr = POT_SPRITES.get(pot_type)
            pot_h = pot_spr['height'] if pot_spr else 0
            pot_w = pot_spr['width'] if pot_spr else 0
        else:
            pot_w = 0
            # Use the plant sprite height so the icon sits above the plant top.
            plant_spr = PLANT_SPRITES.get((plant.get('type'), plant.get('stage', '')))
            pot_h = plant_spr['height'] if plant_spr else 0

        sx = plant['x'] - int(environment.camera_x * parallax)
        sy = plant['y_snap'] - pot_h

        bounce_offset = 0 if self._bounce_t < self._BOUNCE_PERIOD else 2
        icon_x = sx + pot_w // 2 - PLACE_DOWN_ICON['width'] // 2
        icon_y = sy - PLACE_DOWN_ICON['height'] - 2 + bounce_offset
        renderer.draw_sprite_obj(PLACE_DOWN_ICON, icon_x, icon_y)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _follow_camera(self, environment):
        """Pan the camera so the selected plant is roughly centered on screen."""
        plant = self._plants[self._idx]
        layer = plant.get('layer', 'foreground')
        parallax = PARALLAX_FACTORS.get(layer, 1.0)
        # Solve for camera_x such that plant appears at the screen centre:
        #   screen_x = plant['x'] - camera_x * parallax  = DISPLAY_WIDTH // 2
        target_cam = int((plant['x'] - config.DISPLAY_WIDTH // 2) / parallax)
        environment.set_camera(target_cam)
