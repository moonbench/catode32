"""Adoption scene - first-run cat selection and onboarding flow.

States:
  GRID    - 2x2 grid of candidate cats; D-pad to navigate, A to inspect
  PROFILE - per-cat info card; B=back, A=adopt
  CONFIRM - confirmation prompt; B=back, A=confirm
  NAMING  - on-screen keyboard for naming the cat
  MOMENT  - adoption moment: cat walks in, shows love bubble, fades to inside
"""

from scene import Scene
from ui import draw_bubble, Popup
from ui_keyboard import OnScreenKeyboard
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import TOM_ICON, QUEEN_ICON
from assets.character import POSES

_GRID    = 0
_PROFILE = 1
_CONFIRM = 2
_NAMING  = 3
_MOMENT  = 4

_STAR_SIGNS = (
    'Aries', 'Taurus', 'Gemini', 'Cancer', 'Leo', 'Virgo',
    'Libra', 'Scorpio', 'Sagittarius', 'Capricorn', 'Aquarius', 'Pisces',
)

_WEATHER_ADJ = {
    'sunny': 'sunny', 'rainy': 'rainy',
    'snowy': 'snowy', 'overcast': 'cloudy',
}

_TEMPERAMENT_LABELS = ('Bold', 'Loyal', 'Mischievous', 'Curious', 'Sociable')

_PORTRAIT_EXCLUDE = frozenset(('costume_sitting', 'sitting_back'))

_TOM_NAMES    = ('Jasper', 'Orion', 'Bennie', 'Winston', 'Reginald',
                 'Odie', 'Beasley', 'Yoshi', 'Zeus', 'Zeke', 'Leo',
                 'Ajax', 'Java', 'Rio', 'Gizmo', 'Loki', 'Smokey',
                 'Rebel', 'Milo', 'Simba', 'Rocky', 'Jet', 'Mozart',
                 'Spunky', 'Yogi', 'Ollie', 'Otto', 'Skipper', 'Rex')
_QUEEN_NAMES  = ('Bean', 'Lyra', 'Tressym', 'Angel', 'Callie',
                 'Honey', 'Piper', 'Roxie', 'Daisy', 'Jasmine',
                 'Lizzy', 'Daphnie', 'Paprika', 'Mocha', 'Cocoa',
                 'Luna', 'Peaches', 'Kiki', 'Suki', 'Cleo', 'Violet',
                 'Lilith', 'Buffie', 'Piper', 'Star', 'Maya', 'Hidey')
_EITHER_NAMES = ('Juno', 'Jessie', 'Remy', 'Jiji', 'Turtle',
                 'Bandit', 'Fuzzy', 'June', 'Koko', 'Noodle',
                 'Pixel', 'Scratches', 'Scraps', 'Silver', 'Sushi',
                 'Tiger', 'Tux', 'Umi', 'Whiskers', 'Ziggy', 'Patch',
                 'Midnight', 'Gato', 'Hunter', 'Pepper', 'Bengie',
                 'Kitty', 'Snowball', 'Star', 'Artemis', 'Tang',
                 'Titch', 'Rainbow', 'Speedy', 'Lemony', 'Milkshake',
                 'Jingles', 'Muffin', 'Taco', 'Turbo', 'Speedy')

# Grid layout constants (title 9px, two rows of 27px each = 63px total)
_TITLE_H   = 9
_CELL_W    = 64
_CELL_H    = 27


class AdoptionScene(Scene):
    SCENE_NAME = 'adoption'

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._state      = _GRID
        self._candidates = None   # list of 4 dicts: {seed, favs, offsets}
        self._selected   = 0      # 0-3, current grid highlight
        self._viewing    = 0      # which candidate is on the profile/confirm screen
        self._kb             = None
        self._popup          = None
        self._menu           = None
        self._moment_char    = None
        self._portrait_poses = None
        # Adoption moment state
        self._moment_phase = ''
        self._moment_x     = 0.0
        self._moment_timer = 0.0
        self._bubble_prog  = 0.0

    # ------------------------------------------------------------------
    # Scene lifecycle
    # ------------------------------------------------------------------

    def load(self):
        super().load()
        self._kb     = OnScreenKeyboard(self.renderer, self.input, charset='full', max_len=12)
        self._popup  = Popup(self.renderer, x=0, y=0, width=128, height=55, padding=3)
        self._menu   = Menu(self.renderer, self.input)
        self._portrait_poses = self._build_portrait_poses(POSES)
        self._candidates = self._make_candidates()

    @staticmethod
    def _build_portrait_poses(poses):
        result = []
        for position, directions in poses.items():
            if position in _PORTRAIT_EXCLUDE:
                continue
            for direction, emotions in directions.items():
                for emotion, pose in emotions.items():
                    if pose.get('head') and pose.get('eyes'):
                        result.append(position + '.' + direction + '.' + emotion)
        return result

    def _make_candidates(self):
        import reset_context
        import sys
        toms, queens   = [], []
        used_portraits = set()
        used_tom_id    = set()   # (temper_idx, sign_idx) pairs already taken by toms
        used_queen_id  = set()   # same for queens
        while len(toms) < 2 or len(queens) < 2:
            seed    = reset_context._generate_seed()
            favs    = reset_context._derive_favorites(seed)
            offs    = reset_context._derive_trait_offsets(seed)
            gender  = favs['pet_gender']
            portrait_idx = (seed >> 40) % len(self._portrait_poses)
            pname = self._portrait_poses[portrait_idx]
            parts = pname.split('.')
            pose  = POSES[parts[0]][parts[1]][parts[2]]
            portrait_key = (id(pose['head']), id(pose['eyes']))
            identity = (offs.index(max(offs)), favs['star_sign'])  # (temper, star_sign)
            id_set  = used_tom_id if gender == 'tom' else used_queen_id
            if portrait_key in used_portraits or identity in id_set:
                continue
            pool = (_TOM_NAMES if gender == 'tom' else _QUEEN_NAMES) + _EITHER_NAMES
            name = pool[(seed >> 20) % len(pool)]
            cand = {'seed': seed, 'favs': favs, 'offsets': offs, 'name': name}
            if gender == 'tom' and len(toms) < 2:
                toms.append(cand)
                used_portraits.add(portrait_key)
                id_set.add(identity)
            elif gender == 'queen' and len(queens) < 2:
                queens.append(cand)
                used_portraits.add(portrait_key)
                id_set.add(identity)
        sys.modules.pop('reset_context', None)
        import random
        candidates = [toms[0], queens[0], toms[1], queens[1]]
        for i in range(len(candidates) - 1, 0, -1):
            j = random.randint(0, i)
            candidates[i], candidates[j] = candidates[j], candidates[i]
        return candidates

    def unload(self):
        super().unload()
        self._candidates     = None
        self._kb             = None
        self._popup          = None
        self._menu           = None
        self._moment_char    = None
        self._portrait_poses = None

    def enter(self):
        self._state    = _GRID
        self._selected = 0

    def exit(self):
        pass

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if self._state == _MOMENT:
            return self._update_moment(dt)
        return None

    def _update_moment(self, dt):
        if self._moment_phase == 'walking':
            self._moment_x        += 30.0 * dt
            self._moment_char.x    = self._moment_x
            self._moment_char.update(dt)
            if self._moment_x >= 64.0:
                self._moment_x     = 64.0
                self._moment_phase = 'sitting'
                self._moment_timer = 0.0
                self._bubble_prog  = 0.0
                self._moment_char.set_pose('sitting.forward.happy')
        elif self._moment_phase == 'sitting':
            self._moment_timer += dt
            self._bubble_prog   = min(1.0, self._moment_timer / 4.5)
            self._moment_char.update(dt)
            if self._moment_timer >= 5.0:
                return ('change_scene', 'inside')
        return None

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._state == _GRID:
            return self._input_grid()
        if self._state == _PROFILE:
            return self._input_profile()
        if self._state == _CONFIRM:
            return self._input_confirm()
        if self._state == _NAMING:
            return self._input_naming()
        return None

    def _input_grid(self):
        col = self._selected % 2
        row = self._selected // 2
        if self.input.was_just_pressed('up') or self.input.was_just_pressed('down'):
            row = 1 - row
        elif self.input.was_just_pressed('left') or self.input.was_just_pressed('right'):
            col = 1 - col
        self._selected = row * 2 + col
        if self.input.was_just_pressed('a'):
            self._viewing = self._selected
            self._popup.set_text(self._profile_text(self._candidates[self._viewing]))
            self._state   = _PROFILE
        return None

    def _input_profile(self):
        if self.input.was_just_pressed('b'):
            self._state = _GRID
        elif self.input.was_just_pressed('a'):
            item = MenuItem('Adopt', action=('adopt',), confirm='Is this the cat you want to adopt?')
            self._menu.open([item])
            self._menu.pending_confirmation = item
            self._state = _CONFIRM
        elif self.input.was_just_pressed('down'):
            self._popup.scroll_down()
        elif self.input.was_just_pressed('up'):
            self._popup.scroll_up()
        return None

    def _input_confirm(self):
        result = self._menu.handle_input()
        if result == ('adopt',):
            cand = self._candidates[self._viewing]
            self._kb.open('Name your cat', cand['name'])
            self._state = _NAMING
        elif result is not None or not self._menu.pending_confirmation:
            self._state = _PROFILE
        return None

    def _input_naming(self):
        result = self._kb.handle_input()
        if result is not None:
            self._do_adopt(result.strip() or 'Cat')
        return None

    # ------------------------------------------------------------------
    # Adoption
    # ------------------------------------------------------------------

    def _do_adopt(self, name):
        cand = self._candidates[self._viewing]
        ctx  = self.context
        ctx.pet_seed = cand['seed']
        for k, v in cand['favs'].items():
            setattr(ctx, k, v)
        # Apply personality trait offsets (courage/loyalty/etc.)
        offs        = cand['offsets']
        trait_names = ('courage', 'loyalty', 'mischievousness', 'curiosity', 'sociability')
        for i, trait in enumerate(trait_names):
            setattr(ctx, trait, 50 + offs[i])
        ctx.pet_name          = name
        ctx.milestones        = {'fed': False, 'groomed': False, 'played': False,
                                  'petted': False, 'store': False}
        ctx.first_impressions = True
        import backup as _bk
        import sys as _sys
        _bk.write_adoption(self._candidates, cand['seed'], name)
        _sys.modules.pop('backup', None)
        # Kick off the adoption moment
        self._moment_x      = -20.0
        self._moment_timer  = 0.0
        self._bubble_prog   = 0.0
        self._moment_char   = CharacterEntity(x=self._moment_x, y=55, context=self.context)
        self._moment_char.set_pose('walking.side.neutral')
        self._moment_phase  = 'walking'
        self._state         = _MOMENT

    # ------------------------------------------------------------------
    # Draw dispatch
    # ------------------------------------------------------------------

    def draw(self):
        if self._state == _GRID:
            self._draw_grid()
        elif self._state == _PROFILE:
            self._draw_profile()
        elif self._state == _CONFIRM:
            self._draw_confirm()
        elif self._state == _NAMING:
            self._kb.draw()
        elif self._state == _MOMENT:
            self._draw_moment()

    # ------------------------------------------------------------------
    # Grid
    # ------------------------------------------------------------------

    def _draw_grid(self):
        r = self.renderer

        # Title
        title = 'Adoptable Pets'
        r.draw_text(title, (128 - len(title) * 8) // 2, 0)
        # Cells
        pp = self._portrait_poses
        for i, cand in enumerate(self._candidates):
            col = i % 2
            row = i // 2
            cx  = col * _CELL_W
            cy  = _TITLE_H + row * _CELL_H
            self._draw_cell(r, POSES, pp, cand, cx, cy, i == self._selected)

    def _draw_cell(self, r, POSES, portrait_poses, cand, cx, cy, selected):
        # Headshot first so the border renders on top of any sprite overlap
        seed   = cand['seed']
        pname  = portrait_poses[(seed >> 40) % len(portrait_poses)]
        parts  = pname.split('.')
        pose   = POSES[parts[0]][parts[1]][parts[2]]
        head   = pose['head']
        eyes   = pose['eyes']

        # Center head horizontally; 1px from top of cell interior
        hx = cx + (_CELL_W - head['width']) // 2
        hy = cy + 1
        r.draw_sprite_obj(head, hx, hy)
        ex = hx + head['eye_x'] - eyes['anchor_x']
        ey = hy + head['eye_y'] - eyes['anchor_y']
        r.draw_sprite_obj(eyes, ex, ey)

        gender = cand['favs']['pet_gender']
        icon   = TOM_ICON if gender == 'tom' else QUEEN_ICON
        r.draw_sprite_obj(icon, cx + _CELL_W - 12, cy + _CELL_H - 12)

        # Border drawn last so it's always visible over the sprite
        r.draw_rect(cx, cy, _CELL_W - 1, _CELL_H - 1, filled=False, color=1)
        if selected:
            r.draw_rect(cx + 1, cy + 1, _CELL_W - 3, _CELL_H - 3, filled=False, color=1)

    # ------------------------------------------------------------------
    # Profile
    # ------------------------------------------------------------------

    def _profile_text(self, cand):
        favs    = cand['favs']
        offs    = cand['offsets']
        seed    = cand['seed']
        gender     = favs['pet_gender']
        possessive = 'His' if gender == 'tom' else 'Her'
        sign    = _STAR_SIGNS[seed % 12]
        dom     = offs.index(max(offs))
        temper  = _TEMPERAMENT_LABELS[dom]
        weather = _WEATHER_ADJ.get(favs.get('fav_weather', ''), 'any')
        def _fmt(key):
            return key.replace('_', ' ')
        meal  = _fmt(favs['fav_meal'])
        snack = _fmt(favs['fav_snack'])
        toy   = _fmt(favs['fav_toy'])
        line1 = cand['name'] + ' is a ' + temper.lower() + ' ' + sign.lower() + ', who loves ' + weather + ' weather.'
        line2 = possessive + ' favorite meal is ' + meal + '. ' + possessive + ' favorite snack is ' + snack + '. And ' + possessive.lower() + ' favorite toy is the ' + toy + '.'
        return line1 + '\n' + line2

    def _draw_profile(self):
        self._popup.draw()
        self.renderer.draw_text('[A]Adopt [B]Back', 0, 56)

    # ------------------------------------------------------------------
    # Confirm
    # ------------------------------------------------------------------

    def _draw_confirm(self):
        self._menu.draw()

    # ------------------------------------------------------------------
    # Adoption moment
    # ------------------------------------------------------------------

    def _draw_moment(self):
        r = self.renderer

        if self._moment_phase == 'walking':
            self._moment_char.draw(r, mirror=True)
        elif self._moment_phase == 'sitting':
            self._moment_char.draw(r, mirror=False)
            draw_bubble(r, 'exclaim', int(self._moment_x), 55, self._bubble_prog, False)
