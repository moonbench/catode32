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
        self.on_exit()

    def on_exit(self):
        """Override for sky teardown, etc."""
        pass

    def update(self, dt):
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
        self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)
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
                self._handle_menu_action(result)
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
            MenuItem("Kiss", icon=HEART_ICON, action=("kiss",)),
            MenuItem("Psst psst", icon=HEART_BUBBLE_ICON, action=("psst",)),
            MenuItem("Groom", icon=HAND_ICON, action=("groom",))
        ]

        meal_items = [
            MenuItem("Chicken", icon=CHICKEN_ICON, action=("meal", "chicken")),
            MenuItem("Fish", icon=FISH_ICON, action=("meal", "fish")),
        ]
        snack_items = [
            MenuItem(snack["name"], icon=SNACK_ICONS.get(snack["name"]), action=("snack", snack))
            for snack in self.context.inventory.get("snacks", [])
        ]
        feed_items = [
            MenuItem("Meals", icon=MEAL_ICON, submenu=meal_items),
            MenuItem("Snacks", icon=KIBBLE_ICON, submenu=snack_items),
        ]

        toy_items = [
            MenuItem(toy["name"], icon=TOY_ICONS.get(toy["name"]), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
        ]

        train_items = [
            MenuItem("Intelligence", icon=HAND_ICON, action=("train",)),
            MenuItem("Behavior", icon=HAND_ICON, action=("train",)),
            MenuItem("Fitness", icon=HAND_ICON, action=("train",)),
            MenuItem("Sociability", icon=HAND_ICON, action=("train",)),
        ]

        items = [
            MenuItem("Affection", icon=HEART_ICON, submenu=affection_items),
            MenuItem("Train", icon=HAND_ICON, submenu=train_items),
            MenuItem("Feed", icon=MEAL_ICON, submenu=feed_items),
        ]

        if toy_items:
            items.append(MenuItem("Play", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        if not action:
            return

        action_type = action[0]

        if action_type == "meal":
            self.character.trigger('eating', food_sprite=FOOD_BOWL, food_type=action[1])
        elif action_type == "kiss":
            self.character.trigger('affection', variant='kiss')
        elif action_type == "pets":
            self.character.trigger('affection', variant='pets')
        elif action_type == "psst":
            self.character.trigger('attention', variant='psst')
        elif action_type == "snack":
            self.character.trigger('eating', food_sprite=TREAT_PILE, food_type='treat')
        elif action_type == "toy":
            self.character.trigger('playing', variant=action[1]['variant'])
        elif action_type == "groom":
            self.character.trigger('being_groomed')
        elif action_type == "train":
            self.character.trigger('training')
