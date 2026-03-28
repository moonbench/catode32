import config
from scene import Scene
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import (TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON,
                          KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON,
                          CHICKEN_ICON, MEAL_ICON)
from assets.items import FOOD_BOWL, TREAT_PILE


class MainScene(Scene):
    """Base class for main location scenes (inside, outside, etc.).

    Handles the shared pet interaction menu, character rendering, camera
    auto-panning, and the behavior lifecycle around scene enter/exit.

    Subclasses must set SCENE_NAME and implement setup_scene(). They may
    also override the on_* hooks for scene-specific behaviour:

      setup_scene  - create self.environment + self.character, place objects
      on_enter     - add custom draws, configure sky, etc.
      on_exit      - teardown sky, etc.
      on_update    - update sky/entities; must call self.character.update(dt)
      on_pre_draw  - any setup needed before environment.draw()
      on_post_draw - any drawing after the character (e.g. renderer.invert)
    """

    SCENE_NAME = None  # override in subclass
    ENTRY_X = 64      # character x position on scene entry (cached or fresh)

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None
        self.menu = None
        self.visitor_cat = None       # VisitorCatEntity; present only during an active visit
        self._last_broadcast_pose = None  # pose_name sent in last vst broadcast
        self._last_broadcast_x = None    # x sent in last vst broadcast (for position-change trigger)
        self._last_broadcast_vx = 0      # vx sent in last vst broadcast (to detect stop events)
        self._char_vx = 0.0              # pixels/second derived each frame from position delta
        self._vst_timer = 0.0         # seconds until next heartbeat vst
        self._venv_timer = 0.0        # seconds until next environment sync (inviter only, ~1/s)

    def load(self):
        super().load()
        self.setup_scene()
        self.menu = Menu(self.renderer, self.input)

    def setup_scene(self):
        """Override to create self.environment, self.character, and place objects."""
        pass

    def unload(self):
        super().unload()

    def enter(self):
        if self.SCENE_NAME:
            self.context.last_main_scene = self.SCENE_NAME
        if self.character:
            self.character.x = self.ENTRY_X

        # If a visit is active, offset both cats so they start on opposite sides
        if self.context.visit is not None:
            from entities.visitor_cat import VisitorCatEntity
            y = int(self.character.y) if self.character else 64
            if self.context.visit.get('role') == 'inviter':
                self.character.x = self.ENTRY_X - 20
                visitor_x = self.ENTRY_X + 20
            else:
                self.character.x = self.ENTRY_X + 20
                visitor_x = self.ENTRY_X - 20
            self.visitor_cat = VisitorCatEntity(visitor_x, y)

            # Tell the peer which scene we just entered
            if self.SCENE_NAME and self.context.espnow:
                self.context.espnow.send_to(
                    self.context.visit['peer_mac'], 'vloc', {'s': self.SCENE_NAME}
                )
                # Inviter is authoritative for sky — push environment on every scene entry
                # so weather/time changes (which trigger re-enters) propagate to peer
                if self.context.visit.get('role') == 'inviter':
                    env = self.context.environment
                    self.context.espnow.send_to(
                        self.context.visit['peer_mac'], 'venv', {
                            'h':  env.get('time_hours', 12),
                            'mn': env.get('time_minutes', 0),
                            'w':  env.get('weather', 'Clear'),
                            's':  env.get('season', 'Spring'),
                            'mp': env.get('moon_phase', 'Full'),
                        }
                    )

        self.on_enter()
        if self.character and not self.character.current_behavior.active:
            self.character.behavior_manager.resume_prior_behavior()

    def on_enter(self):
        """Override to configure sky, add custom draws, etc."""
        pass

    def exit(self):
        if self.character:
            self.character.behavior_manager.stop_current()
        self.environment.custom_draws.clear()
        self.visitor_cat = None
        self._last_broadcast_pose = None
        self._last_broadcast_x = None
        self._last_broadcast_vx = 0
        self._char_vx = 0.0
        self._venv_timer = 0.0
        self.on_exit()

    def on_exit(self):
        """Override for sky teardown, etc."""
        pass

    def update(self, dt):
        prev_x = self.character.x
        self.on_update(dt)
        self._char_vx = (self.character.x - prev_x) / dt if dt > 0 else 0.0
        if not (self.input.is_pressed('left') or self.input.is_pressed('right')):
            if int(prev_x) != int(self.character.x):
                margin = 32
                screen_x = int(self.character.x) - int(self.environment.camera_x)
                if screen_x < margin:
                    self.environment.set_camera(int(self.character.x) - margin)
                elif screen_x > config.DISPLAY_WIDTH - margin:
                    self.environment.set_camera(int(self.character.x) - (config.DISPLAY_WIDTH - margin))
        self._update_visit(dt)

    def _update_visit(self, dt):
        """Tick visitor cat animation and sync our state to the peer."""
        sky = getattr(self, 'sky', None)
        if self.context.visit is not None:
            if self.visitor_cat is None:
                from entities.visitor_cat import VisitorCatEntity
                y = int(self.character.y) if self.character else 64
                self.visitor_cat = VisitorCatEntity(96, y)
            self.visitor_cat.update(dt)
            self._broadcast_vst(dt)
            self._broadcast_venv(dt)
            if sky:
                is_inviter = self.context.visit.get('role') == 'inviter'
                sky.suppress_auto_spawns = not is_inviter
                if sky.pending_events:
                    if is_inviter and self.context.espnow:
                        for evt_type, params in sky.pending_events:
                            msg = 'vss' if evt_type == 'ss' else 'vse'
                            self.context.espnow.send_to(
                                self.context.visit['peer_mac'], msg, params)
                    sky.pending_events.clear()
        else:
            if self.visitor_cat is not None:
                # Visit was ended remotely; clear the entity
                self.visitor_cat = None
            if sky:
                sky.suppress_auto_spawns = False
                sky.pending_events.clear()

    def _broadcast_venv(self, dt):
        """Inviter sends time update once per second to keep peer's sky in sync.

        Weather, season, and moon phase are only sent once on scene entry (see enter()).
        """
        if self.context.visit is None or self.context.visit.get('role') != 'inviter':
            return
        self._venv_timer -= dt
        if self._venv_timer > 0:
            return
        self._venv_timer = 1.0
        if self.context.espnow:
            env = self.context.environment
            self.context.espnow.send_to(
                self.context.visit['peer_mac'], 'venv', {
                    'h':  env.get('time_hours', 12),
                    'mn': env.get('time_minutes', 0),
                }
            )

    def _broadcast_vst(self, dt):
        """Send our cat's current state to the peer, on pose change, position change (≥4px), velocity stop, or every 3s."""
        self._vst_timer -= dt
        current_x = int(self.character.x)
        current_pose = self.character.pose_name
        current_vx = int(self._char_vx)
        pos_changed = (self._last_broadcast_x is None or
                       abs(current_x - self._last_broadcast_x) >= 4)
        just_stopped = (current_vx == 0 and self._last_broadcast_vx != 0)
        if current_pose != self._last_broadcast_pose or self._vst_timer <= 0 or pos_changed or just_stopped:
            if self.context.espnow and self.context.visit:
                payload = {'x': current_x, 'p': current_pose,
                           'm': 1 if self.character.mirror else 0}
                if current_vx != 0:
                    payload['vx'] = current_vx
                self.context.espnow.send_to(
                    self.context.visit['peer_mac'], 'vst', payload,
                )
            self._last_broadcast_pose = current_pose
            self._last_broadcast_x = current_x
            self._last_broadcast_vx = current_vx
            self._vst_timer = 3.0

    def on_update(self, dt):
        """Override for sky/entity updates. Must call self.character.update(dt)."""
        self.character.update(dt)

    def draw(self):
        if self.menu_active:
            self.menu.draw()
            return
        self.on_pre_draw()
        self.environment.draw(self.renderer)
        camera_offset = int(self.environment.camera_x)
        # Inviter is always drawn in front (last). Visitor draws behind (first).
        inviter_is_us = (self.context.visit is None or
                         self.context.visit.get('role') == 'inviter')
        if inviter_is_us:
            if self.visitor_cat is not None:
                self.visitor_cat.draw(self.renderer, camera_offset=camera_offset)
            self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)
        else:
            self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)
            if self.visitor_cat is not None:
                self.visitor_cat.draw(self.renderer, camera_offset=camera_offset)
        self.on_post_draw()

    def on_pre_draw(self):
        """Override for any setup needed before environment.draw()."""
        pass

    def on_post_draw(self):
        """Override for any drawing after the character (e.g. renderer.invert)."""
        pass

    def handle_input(self):
        if self.menu_active:
            result = self.menu.handle_input()
            if result == 'closed':
                self.menu_active = False
            elif result is not None:
                self.menu_active = False
                return self._handle_menu_action(result)
            return None

        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.menu.open(self._build_menu_items())
            return None

        dx, dy = self.input.get_direction()
        if dx != 0:
            self.environment.pan(dx * config.PAN_SPEED)

        return None

    def _build_menu_items(self):
        affection_items = [
            MenuItem("Pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Scratch", icon=HAND_ICON, action=("scratch",)),
            MenuItem("Kiss", icon=HEART_ICON, action=("kiss",)),
            MenuItem("Psst psst", icon=HEART_BUBBLE_ICON, action=("psst",)),
            MenuItem("Groom", icon=HAND_ICON, action=("groom",))
        ]

        food_stock = self.context.food_stock
        _meal_defs = (
            ("Chicken",  "chicken",  CHICKEN_ICON),
            ("Salmon",   "salmon",   FISH_ICON),
            ("Tuna",     "tuna",     FISH_ICON),
            ("Shrimp",   "shrimp",   FISH_ICON),
            ("Trout",    "trout",    FISH_ICON),
            ("Herring",  "herring",  FISH_ICON),
            ("Haddock",  "haddock",  FISH_ICON),
            ("Cod",      "cod",      FISH_ICON),
            ("Mackerel", "mackerel", FISH_ICON),
            ("Turkey",   "turkey",   CHICKEN_ICON),
            ("Beef",     "beef",     MEAL_ICON),
            ("Lamb",     "lamb",     MEAL_ICON),
            ("Liver",    "liver",    MEAL_ICON),
            ("Kibble",   "kibble",   KIBBLE_ICON),
        )
        meal_items = [
            MenuItem(f"{name} ({food_stock.get(key, 0)})", icon=icon, action=("meal", key))
            for name, key, icon in _meal_defs
            if food_stock.get(key, 0) > 0
        ]
        _snack_defs = (
            ("Treats",     "treats"),
            ("Chew Stick", "chew_stick"),
            ("Nugget",     "nugget"),
            ("Puree",      "puree"),
            ("Cream",      "cream"),
            ("Milk",       "milk"),
            ("Fish Bite",  "fish_bite"),
            ("Eggs",       "eggs"),
            ("Pumpkin",    "pumpkin"),
            ("Carrots",    "carrots"),
        )
        snack_items = [
            MenuItem(f"{name} ({food_stock.get(key, 0)})",
                     icon=SNACK_ICONS.get(name, KIBBLE_ICON),
                     action=("snack", key))
            for name, key in _snack_defs
            if food_stock.get(key, 0) > 0
        ]
        feed_items = []
        if meal_items:
            feed_items.append(MenuItem("Meals", icon=MEAL_ICON, submenu=meal_items))
        if snack_items:
            feed_items.append(MenuItem("Snacks", icon=KIBBLE_ICON, submenu=snack_items))
        feed_items.append(MenuItem("Store...", action=("go_store",)))

        toy_items = [
            MenuItem(toy["name"], icon=TOY_ICONS.get(toy["name"]), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
        ]
        toy_items.append(MenuItem("Store...", action=("go_store",)))

        train_items = [
            MenuItem("Intelligence", icon=HAND_ICON, action=("train",)),
            MenuItem("Behavior", icon=HAND_ICON, action=("train",)),
            MenuItem("Fitness", icon=HAND_ICON, action=("train",)),
            MenuItem("Sociability", icon=HAND_ICON, action=("train",)),
        ]

        items = [
            MenuItem("Affection", icon=HEART_ICON, submenu=affection_items),
            MenuItem("Train", icon=HAND_ICON, submenu=train_items),
        ]
        items.append(MenuItem("Feed", icon=MEAL_ICON, submenu=feed_items))
        items.append(MenuItem("Play", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        if not action:
            return

        action_type = action[0]

        if action_type == "meal":
            food_type = action[1]
            self.character.trigger('eating', food_sprite=FOOD_BOWL, food_type=food_type)
            self.context.food_stock[food_type] = max(0, self.context.food_stock.get(food_type, 0) - 1)
        elif action_type == "kiss":
            self.character.trigger('affection', variant='kiss')
        elif action_type == "pets":
            self.character.trigger('affection', variant='pets')
        elif action_type == "scratch":
            self.character.trigger('affection', variant='scratching')
        elif action_type == "psst":
            self.character.trigger('attention', variant='psst')
        elif action_type == "snack":
            snack_key = action[1]
            self.character.trigger('eating', food_sprite=TREAT_PILE, food_type=snack_key)
            self.context.food_stock[snack_key] = max(0, self.context.food_stock.get(snack_key, 0) - 1)
        elif action_type == "toy":
            self.character.trigger('playing', variant=action[1]['variant'])
        elif action_type == "groom":
            self.character.trigger('being_groomed')
        elif action_type == "train":
            self.character.trigger('training')
        elif action_type == "go_store":
            return ('change_scene', 'store')
