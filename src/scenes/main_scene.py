import random
import config
from scene import Scene
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from plant_system import (tick_plants, plant_seed, water_plant, fertilize_plant,
                          remove_plant, repot_plant, move_plant, get_plant_by_id,
                          inspect_lines, plant_in_ground)
from plant_renderer import register_plant_draws, invalidate_plant_cache
from gardening_ui import PlacementMode, PlantSelectionMode
from assets.icons import (TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON,
                          KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON,
                          CHICKEN_ICON, MEAL_ICON, TREES_ICON)
from assets.items import FOOD_BOWL, TREAT_PILE
from ui import draw_bubble, Popup


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
        self._placement = PlacementMode()
        self._plant_selection = PlantSelectionMode()
        self._in_tend_mode = False
        self._last_tended_plant_id = None
        self._popup_msg = None
        self._plant_bursts = {}  # {plant_id: {'timer': float, 'bursts': [{dx,dy,delay},...]}}

    def load(self):
        super().load()
        self.setup_scene()
        self.menu = Menu(self.renderer, self.input)
        self._popup = Popup(self.renderer, x=14, y=12, width=100, height=40, padding=4)

    def setup_scene(self):
        """Override to create self.environment, self.character, and place objects."""
        pass

    def unload(self):
        super().unload()

    def enter(self):
        if self.SCENE_NAME:
            self.context.last_main_scene = self.SCENE_NAME
        if self.character:
            saved_x = getattr(self.context, 'saved_cat_x', None)
            x_min = getattr(self.context, 'scene_x_min', 10)
            x_max = getattr(self.context, 'scene_x_max', 118)
            if saved_x is not None and x_min <= saved_x <= x_max:
                self.character.x = saved_x
            else:
                self.character.x = self.ENTRY_X

        # Offset cats to opposite sides when a visit is active
        if self.context.visit is not None:
            if self.context.visit.get('role') == 'inviter':
                self.character.x = self.ENTRY_X - 20
            else:
                self.character.x = self.ENTRY_X + 20

        vm = getattr(self.context, 'visit_manager', None)
        if vm:
            vm.on_scene_enter(self)

        self.on_enter()

        if getattr(self, 'PLANT_SURFACES', None):
            register_plant_draws(self)

        # Handle pending plant move arriving at this scene.
        move = getattr(self.context, 'pending_gardening_move', None)
        if move and move.get('dest_scene') == self.SCENE_NAME:
            self.context.pending_gardening_move = None
            plant_id = move['plant_id']
            plant = get_plant_by_id(self.context, plant_id)
            if plant and plant.get('pot') != 'ground':
                def _on_cross_scene_move(layer, x, y_snap, _pid=plant_id):
                    move_plant(self.context, _pid, self.SCENE_NAME, layer, x, y_snap)
                self._placement.enter(plant['pot'], self, on_confirm=_on_cross_scene_move)

        if self.character:
            self.character.behavior_manager.resume_prior_behavior()

    def on_enter(self):
        """Override to configure sky, add custom draws, etc."""
        pass

    def exit(self):
        tick_plants(self.context)
        if self.character:
            self.context.saved_cat_x = self.character.x
            self.character.behavior_manager.stop_current()
        self.environment.custom_draws.clear()
        vm = getattr(self.context, 'visit_manager', None)
        if vm:
            vm.on_scene_exit()
        self.on_exit()

    def on_exit(self):
        """Override for sky teardown, etc."""
        pass

    def update(self, dt):
        if self._placement.active:
            self._placement.update(dt)
        if self._plant_selection.active:
            self._plant_selection.update(dt)
        prev_x = self.character.x
        self.on_update(dt)
        if not (self.input.is_pressed('left') or self.input.is_pressed('right')):
            if int(prev_x) != int(self.character.x):
                margin = 32
                screen_x = int(self.character.x) - int(self.environment.camera_x)
                if screen_x < margin:
                    self.environment.set_camera(int(self.character.x) - margin)
                elif screen_x > config.DISPLAY_WIDTH - margin:
                    self.environment.set_camera(int(self.character.x) - (config.DISPLAY_WIDTH - margin))
        self._check_lightning_startled()
        if self._plant_bursts:
            for info in self._plant_bursts.values():
                info['timer'] += dt
            expired = [pid for pid, info in self._plant_bursts.items() if info['timer'] >= 3.0]
            for pid in expired:
                del self._plant_bursts[pid]

    # Behaviors that block lightning startled (don't interrupt deep sleep etc.)
    _NO_STARTLE_BEHAVIORS = frozenset(('sleeping', 'eating', 'being_groomed', 'training'))

    def _check_lightning_startled(self):
        """Trigger startled when lightning strikes, regardless of playdate state."""
        sky = getattr(self, 'sky', None)
        if not sky or not getattr(sky, 'lightning_just_started', False):
            return
        cb = self.character.current_behavior
        if cb and cb.NAME in self._NO_STARTLE_BEHAVIORS:
            return
        # Probability scales with inverse courage (same curve as can_trigger_startled)
        # Location modifier: exposed locations increase chance, indoors reduces it
        scene = getattr(self, 'SCENE_NAME', '')
        if scene in ('outside', 'treehouse'):
            location_mul = 1.4
        elif scene == 'inside':
            location_mul = 0.5
        else:
            location_mul = 1.0
        ctx = self.character.context
        p = min(1.0, 0.6 * (1 - ctx.courage / 100) * location_mul)
        if random.random() < p:
            print('[Scene] Lightning startled! p=%.2f courage=%.1f loc=%s' % (p, ctx.courage, scene))
            self.character.trigger('startled')

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

        vm = getattr(self.context, 'visit_manager', None)
        visitor_cat = vm.visitor_cat if vm else None

        # Inviter is always drawn in front (last). Visitor draws behind (first).
        inviter_is_us = (self.context.visit is None or
                         self.context.visit.get('role') == 'inviter')
        if inviter_is_us:
            if visitor_cat is not None:
                visitor_cat.draw(self.renderer, camera_offset=camera_offset)
            self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)
        else:
            self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)
            if visitor_cat is not None:
                visitor_cat.draw(self.renderer, camera_offset=camera_offset)

        # Draw visitor cat's speech bubble (vocalization exchange, greeting, sniff)
        if vm and vm.visitor_bubble is not None and visitor_cat is not None:
            icon, remaining, max_secs = vm.visitor_bubble
            progress = 1.0 - (remaining / max_secs) if max_secs > 0 else 0.0
            vis_x = int(visitor_cat.x) - camera_offset
            vis_y = int(visitor_cat.y)
            draw_bubble(self.renderer, icon, vis_x, vis_y, progress, visitor_cat.mirror)

        self.on_post_draw()

        if self._placement.active:
            self._placement.draw(self.renderer, self.environment)
        if self._plant_selection.active:
            self._plant_selection.draw(self.renderer, self.environment)
        if self._popup_msg is not None:
            self._popup.draw()

    def on_pre_draw(self):
        """Override for any setup needed before environment.draw()."""
        pass

    def on_post_draw(self):
        """Override for any drawing after the character (e.g. renderer.invert)."""
        pass

    # ------------------------------------------------------------------

    # Scenes that support plant placement (have PLANT_SURFACES defined).
    _PLANTABLE_SCENES = (
        ('inside',    'Inside'),
        ('kitchen',   'Kitchen'),
        ('outside',   'Outside'),
        ('bedroom',   'Bedroom'),
        ('treehouse', 'Treehouse'),
    )

    # Tend submenu actions that should re-enter plant selection when done.
    # tend_move_here is intentionally absent: placement-mode ending handles
    # the re-entry for within-scene moves (see handle_input placement block).
    _TEND_REENTER_ACTIONS = ('tend_pluck', 'tend_repot', 'tend_water', 'tend_fertilize', 'inspect_dismiss')

    def handle_input(self):
        if self._popup_msg is not None:
            if self.input.was_just_pressed('a') or self.input.was_just_pressed('b'):
                self._popup_msg = None
            return None

        if self._placement.active:
            self._placement.handle_input(self.input, self.environment)
            if not self._placement.active:
                invalidate_plant_cache(self)
                # After a within-scene move (or cancelled move), re-enter tend selection.
                if self._in_tend_mode:
                    if not self._reenter_tend_selection():
                        self._in_tend_mode = False
            return None

        if self._plant_selection.active:
            self._plant_selection.handle_input(self.input, self.environment)
            # If selection ended without opening a tend submenu, the player
            # pressed B → exit tend mode entirely.
            if not self._plant_selection.active and not self.menu_active:
                self._in_tend_mode = False
            return None

        if self.menu_active:
            result = self.menu.handle_input()
            if result == 'closed':
                self.menu_active = False
                if self._in_tend_mode:
                    if not self._reenter_tend_selection():
                        self._in_tend_mode = False
            elif result is not None:
                self.menu_active = False
                ret = self._handle_menu_action(result)
                if self._in_tend_mode and result[0] in self._TEND_REENTER_ACTIONS:
                    if not self._reenter_tend_selection():
                        self._in_tend_mode = False
                    return None
                return ret
            return None

        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.menu.open(self._build_menu_items())
            return None

        # Suppress camera panning while the player is steering an interactive toy
        behavior = self.character and self.character.current_behavior
        player_controls_toy = (
            behavior
            and getattr(behavior, 'active', False)
            and getattr(behavior, '_variant', None) in ('laser', 'ball', 'string', 'feather')
        )
        dx, dy = self.input.get_direction()
        if dx != 0 and not player_controls_toy:
            self.environment.pan(dx * config.PAN_SPEED)

        return None

    def _reenter_tend_selection(self):
        """Re-enter plant selection for the next tend action. Returns True if
        at least one plant is available, False if the scene is now empty."""
        def _on_tend_selected(plant):
            self._last_tended_plant_id = plant['id']
            self.menu_active = True
            self.menu.open(self._build_tend_items(plant))
        return self._plant_selection.enter(self, _on_tend_selected,
                                           start_plant_id=self._last_tended_plant_id)

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

        inv = self.context.inventory
        _pot_defs = (
            ("Small pot",   "small"),
            ("Medium pot",  "medium"),
            ("Large pot",   "large"),
            ("Planter box", "planter"),
        )
        place_pot_items = [
            MenuItem(f"{name} ({inv['pots'].get(key, 0)})", icon=TREES_ICON,
                     action=("gardening_place_pot", key))
            for name, key in _pot_defs
            if inv['pots'].get(key, 0) > 0
        ]

        _seed_defs = (
            ("Cat Grass",  "cat_grass"),
            ("Sunflower",  "sunflower"),
            ("Rose",       "rose"),
            ("Freesia",    "freesia"),
        )
        in_pot_items = [
            MenuItem(f"{name} ({inv['seeds'].get(key, 0)})", icon=TREES_ICON,
                     action=("gardening_plant_seed", key))
            for name, key in _seed_defs
            if inv['seeds'].get(key, 0) > 0
        ]

        has_spade = inv.get('tools', {}).get('spade', False)
        in_ground_items = []
        if has_spade and getattr(self, 'SCENE_NAME', None) == 'outside':
            in_ground_items = [
                MenuItem(f"{name} ({inv['seeds'].get(key, 0)})", icon=TREES_ICON,
                         action=("gardening_plant_ground", key))
                for name, key in _seed_defs
                if inv['seeds'].get(key, 0) > 0
            ]

        plant_seed_submenu = []
        if in_pot_items:
            plant_seed_submenu.append(MenuItem("In Pot", icon=TREES_ICON, submenu=in_pot_items))
        if in_ground_items:
            plant_seed_submenu.append(MenuItem("In Ground", icon=TREES_ICON, submenu=in_ground_items))

        gardening_items = []
        gardening_items.append(MenuItem("Tend",  icon=TREES_ICON, action=("gardening_tend",)))
        if place_pot_items:
            gardening_items.append(MenuItem("Place Pot",  icon=TREES_ICON, submenu=place_pot_items))
        if plant_seed_submenu:
            gardening_items.append(MenuItem("Plant Seed", icon=TREES_ICON, submenu=plant_seed_submenu))
        gardening_items.append(MenuItem("Store...", action=("go_store",)))

        items = [
            MenuItem("Affection", icon=HEART_ICON, submenu=affection_items),
            MenuItem("Train", icon=HAND_ICON, submenu=train_items),
        ]
        items.append(MenuItem("Feed", icon=MEAL_ICON, submenu=feed_items))
        items.append(MenuItem("Play", icon=TOYS_ICON, submenu=toy_items))
        items.append(MenuItem("Gardening", icon=TREES_ICON, submenu=gardening_items))

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
        elif action_type == "gardening_place_pot":
            self._placement.enter(action[1], self)
        elif action_type == "gardening_plant_seed":
            seed_type = action[1]
            def _on_pot_selected(plant, _st=seed_type):
                plant_seed(self.context, plant['id'], _st)
            found = self._plant_selection.enter(self, _on_pot_selected,
                                                filter_fn=lambda p: p['stage'] == 'empty_pot')
            if not found:
                self._popup_msg = "No empty pots in this location"
                self._popup.set_text(self._popup_msg, center=True)
        elif action_type == "gardening_plant_ground":
            seed_type = action[1]
            scene = self
            def _on_ground_placed(layer, x, y_snap, _st=seed_type, _sc=scene):
                plant_in_ground(_sc.context, _sc.SCENE_NAME, layer, x, y_snap, _st)
                invalidate_plant_cache(_sc)
            self._placement.enter('ground', self, on_confirm=_on_ground_placed)
        elif action_type == "tend_water":
            plant = get_plant_by_id(self.context, action[1])
            if plant:
                water_plant(plant)
                self._plant_bursts[plant['id']] = {
                    'timer': 0.0,
                    'bursts': [
                        {'dx': random.randint(-12, 12), 'dy': random.randint(-25, 5), 'delay': i * 0.4 + random.uniform(0.0, 0.2)}
                        for i in range(4)
                    ]
                }
        elif action_type == "tend_fertilize":
            plant = get_plant_by_id(self.context, action[1])
            if plant:
                fertilize_plant(plant)
                inv = self.context.inventory
                inv['fertilizer'] = max(0, inv.get('fertilizer', 0) - 1)
                self._plant_bursts[plant['id']] = {
                    'timer': 0.0,
                    'bursts': [
                        {'dx': random.randint(-12, 12), 'dy': random.randint(-25, 5), 'delay': i * 0.4 + random.uniform(0.0, 0.2)}
                        for i in range(4)
                    ]
                }
        elif action_type == "gardening_tend":
            self._in_tend_mode = True
            if not self._reenter_tend_selection():
                self._in_tend_mode = False
        elif action_type == "tend_move_here":
            plant = get_plant_by_id(self.context, action[1])
            if plant and plant.get('pot') != 'ground':
                def _on_move_placed(layer, x, y_snap, _pid=action[1]):
                    move_plant(self.context, _pid, self.SCENE_NAME, layer, x, y_snap)
                self._placement.enter(plant['pot'], self, on_confirm=_on_move_placed)
        elif action_type == "tend_move_to":
            self._in_tend_mode = False
            self.context.pending_gardening_move = {'plant_id': action[1], 'dest_scene': action[2]}
            return ('change_scene', action[2])
        elif action_type == "tend_pluck":
            remove_plant(self.context, action[1])
            invalidate_plant_cache(self)
        elif action_type == "tend_repot":
            repot_plant(self.context, action[1], action[2])
        elif action_type == "inspect_dismiss":
            pass  # player pressed A to exit the info panel — nothing to do

    def _build_tend_items(self, plant):
        """Build the tend submenu items for the given plant."""
        _cap_order = ('small', 'medium', 'large', 'planter')
        _pot_labels = {
            'small': 'Small pot', 'medium': 'Medium pot',
            'large': 'Large pot', 'planter': 'Planter box',
        }
        _large_stages = ('mature', 'thriving')
        items = []

        # Inspect: read-only submenu showing stage, health, and water status
        info_items = [
            MenuItem(line, action=("inspect_dismiss",))
            for line in inspect_lines(plant)
        ]

        # Get information about the health of the plant
        items.append(MenuItem("Inspect", icon=TREES_ICON, submenu=info_items))

        # Water: only for plants that are alive and in the ground/pot
        stage = plant.get('stage', '')
        if stage not in ('empty_pot', 'dead'):
            items.append(MenuItem("Water", icon=TREES_ICON, action=("tend_water", plant['id'])))
            if self.context.inventory.get('fertilizer', 0) > 0:
                items.append(MenuItem("Fertilize", icon=TREES_ICON, action=("tend_fertilize", plant['id'])))

        # Move (not available for ground plants)
        if plant.get('pot') != 'ground':
            move_items = self._build_move_items(plant)
            items.append(MenuItem("Move", icon=TREES_ICON, submenu=move_items))

        # Repot submenu: larger pots always allowed; smaller pots only for
        # seedling/young/growing stages that still fit.
        current_pot = plant.get('pot', 'small')
        if current_pot in _cap_order:
            stage = plant.get('stage', '')
            is_large = stage in _large_stages
            inv_pots = self.context.inventory.get('pots', {})
            repot_items = []
            for pt in _cap_order:
                if pt == current_pot:
                    continue
                pt_rank = _cap_order.index(pt)
                cur_rank = _cap_order.index(current_pot)
                if pt_rank < cur_rank and is_large:
                    continue  # mature/thriving plants can't go back to smaller pots
                if inv_pots.get(pt, 0) > 0:
                    repot_items.append(MenuItem(
                        _pot_labels.get(pt, pt),
                        icon=TREES_ICON,
                        action=("tend_repot", plant['id'], pt),
                    ))
            if repot_items:
                items.append(MenuItem("Repot", icon=TREES_ICON, submenu=repot_items))

        # Pluck: confirm only if the plant is alive (not already dead/empty)
        needs_confirm = stage not in ('dead', 'empty_pot')
        items.append(MenuItem(
            "Pluck",
            icon=TREES_ICON,
            action=("tend_pluck", plant['id']),
            confirm="Remove plant?" if needs_confirm else None,
        ))

        return items

    def _build_move_items(self, plant):
        """Build the Move submenu: 'Here' (within scene) + one item per other plantable scene."""
        items = [MenuItem("Around Here", icon=TREES_ICON,
                          action=("tend_move_here", plant["id"]))]
        for scene_name, label in self._PLANTABLE_SCENES:
            if scene_name != self.SCENE_NAME:
                items.append(MenuItem(
                    "To " + label, icon=TREES_ICON,
                    action=("tend_move_to", plant["id"], scene_name),
                ))
        return items
