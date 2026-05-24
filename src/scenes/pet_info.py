"""pet_info.py - View and edit pet name; view pet info."""

from scene import Scene
from menu import Menu, MenuItem
from ui_keyboard import OnScreenKeyboard

_STATE_MENU     = 0
_STATE_EDITING  = 1
_STATE_PORTRAIT = 2

_PORTRAIT_EXCLUDE = frozenset(('costume_sitting', 'sitting_back'))

_TRAIT_NAMES        = ('courage', 'loyalty', 'mischievousness', 'curiosity', 'sociability')
_TEMPERAMENT_LABELS = ('Bold', 'Loyal', 'Mischievous', 'Curious', 'Sociable')


class PetInfoScene(Scene):
    SCENE_NAME = 'pet_info'

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._menu           = Menu(renderer, input)
        self._state          = _STATE_MENU
        self._kb_name        = OnScreenKeyboard(renderer, input, charset='full', max_len=12)
        self._portrait_poses = None

    def load(self):
        super().load()

    def unload(self):
        super().unload()
        self._portrait_poses = None

    def enter(self):
        self._state = _STATE_MENU
        self._rebuild_menu()

    def exit(self):
        pass

    def update(self, dt):
        return None

    def draw(self):
        if self._state == _STATE_EDITING:
            self._kb_name.draw()
        elif self._state == _STATE_PORTRAIT:
            self._draw_portrait()
        else:
            self._menu.draw()

    def handle_input(self):
        if self._state == _STATE_EDITING:
            result = self._kb_name.handle_input()
            if result is not None:
                value = result.strip()
                if value:
                    self.context.pet_name = value
                self._state = _STATE_MENU
                self._rebuild_menu()
            return None

        if self._state == _STATE_PORTRAIT:
            if self.input.was_just_pressed('b'):
                self._state = _STATE_MENU
                self._rebuild_menu()
            return None

        result = self._menu.handle_input()
        if result == 'closed':
            return ('change_scene', 'last_main')
        if result == ('edit',):
            self._kb_name.open('', self.context.pet_name or '')
            self._state = _STATE_EDITING
        elif result == ('portrait',):
            self._state = _STATE_PORTRAIT
        return None

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    @staticmethod
    def _display_name(key):
        _TOY_NAMES = {'ball': 'Yarn Ball'}
        if key in _TOY_NAMES:
            return _TOY_NAMES[key]
        s = key.replace('_', ' ')
        return ' '.join(w[0].upper() + w[1:] for w in s.split(' ') if w)

    def _temperament(self):
        ctx  = self.context
        vals = [getattr(ctx, t, 50) for t in _TRAIT_NAMES]
        return _TEMPERAMENT_LABELS[vals.index(max(vals))]

    def _get_portrait_poses(self):
        if self._portrait_poses is None:
            from assets.character import POSES
            result = []
            for position, directions in POSES.items():
                if position in _PORTRAIT_EXCLUDE:
                    continue
                for direction, emotions in directions.items():
                    for emotion, pose in emotions.items():
                        if pose.get('head') and pose.get('eyes'):
                            result.append(position + '.' + direction + '.' + emotion)
            self._portrait_poses = result
        return self._portrait_poses

    def _draw_portrait(self):
        r    = self.renderer
        ctx  = self.context
        from assets.character import POSES
        pp    = self._get_portrait_poses()
        pname = pp[(ctx.pet_seed >> 40) % len(pp)]
        parts = pname.split('.')
        pose  = POSES[parts[0]][parts[1]][parts[2]]
        head  = pose['head']
        eyes  = pose['eyes']

        hx = (128 - head['width']) // 2
        hy = 10
        r.draw_sprite_obj(head, hx, hy)
        r.draw_sprite_obj(eyes, hx + head['eye_x'] - eyes['anchor_x'],
                                hy + head['eye_y'] - eyes['anchor_y'])

        name = ctx.pet_name or '?'
        r.draw_text(name, (128 - len(name) * 8) // 2, hy + head['height'] + 4)

    def _rebuild_menu(self):
        ctx = self.context
        days = ctx.environment.get('day_number', 0)
        days_str = str(min(days, 9999999)) + ' days'
        gender = getattr(ctx, 'pet_gender', None)
        sign   = getattr(ctx, 'star_sign', None)
        dn = self._display_name
        self._menu.open([
            MenuItem('Name: ' + (ctx.pet_name or '?'), action=('edit',)),
            MenuItem('Age: ' + days_str),
            MenuItem('Gender: ' + (self._display_name(gender) if gender else '?')),
            MenuItem(sign or '?'),
            MenuItem(self._temperament()),
            MenuItem('View Portrait', action=('portrait',)),
            MenuItem('- Favorites -'),
            MenuItem('Meal: ' + dn(ctx.fav_meal)),
            MenuItem('Snack: ' + dn(ctx.fav_snack)),
            MenuItem('Toy: ' + dn(ctx.fav_toy)),
            MenuItem('Place: ' + dn(ctx.fav_location)),
            MenuItem('Weather: ' + dn(ctx.fav_weather)),
        ])
