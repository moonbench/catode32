"""pet_info.py - Rich scrollable pet biography page."""

from lang import t
from scene import Scene
from ui_keyboard import OnScreenKeyboard
from assets.icons import UP_ICON, DOWN_ICON


_PORTRAIT_EXCLUDE = frozenset(('costume_sitting', 'sitting_back'))

_TRAIT_NAMES        = ('courage', 'loyalty', 'mischievousness', 'curiosity', 'sociability')
_TEMPERAMENT_LABELS = (t('Bold'), t('Loyal'), t('Mischievous'), t('Curious'), t('Sociable'))

_VISIBLE  = 8   # 8 × 8 px = 64 px full screen height
_FULL_CPL = 14  # chars per full-width line; leaves room for scroll arrows

_DISPLAY_NAME_OVERRIDES = {'ball': t('Yarn Ball')}

_TRANSLATED_NAMES = {
    'cod': t('Cod'), 'haddock': t('Haddock'), 'trout': t('Trout'),
    'shrimp': t('Shrimp'), 'herring': t('Herring'), 'turkey': t('Turkey'),
    'tuna': t('Tuna'), 'salmon': t('Salmon'), 'chicken': t('Chicken'),
    'liver': t('Liver'), 'beef': t('Beef'), 'lamb': t('Lamb'),
    'carrots': t('Carrots'), 'pumpkin': t('Pumpkin'),
    'treats': t('Treats'), 'bytes': t('Bytes'), 'eggs': t('Eggs'),
    'nuggets': t('Nuggets'), 'milk': t('Milk'), 'sticks': t('Sticks'),
    'puree': t('Puree'), 'kibble': t('Kibble'),
    'string': t('String'), 'feather': t('Feather'), 'mouse': t('Mouse'),
    'ball': t('Yarn Ball'), 'yarn': t('Yarn'), 'laser': t('Laser'),
    'living_room': t('Living Room'), 'bedroom': t('Bedroom'),
    'kitchen': t('Kitchen'), 'outside': t('Outside'),
    'treehouse': t('Treehouse'),
    'sunny': t('sunny'), 'rainy': t('rainy'),
    'snowy': t('snowy'), 'overcast': t('cloudy'),
}

# (stat_key, low_threshold, sentence_template)
# {s} = She/He  {h} = her/his
_MOOD_CHECKS = (
    ('health',          35, t("{s}'s not feeling {h} best.")),
    ('fullness',        30, t("{s}'s feeling hungry.")),
    ('energy',          30, t("{s}'s feeling tired.")),
    ('comfort',         30, t("{s} seems uncomfortable.")),
    ('cleanliness',     25, t("{s}'s feeling grubby.")),
    ('fitness',         20, t("{s}'s feeling sluggish.")),
    ('focus',           25, t("{s}'s feeling scattered.")),
    ('intelligence',    20, t("{s}'s understimulated.")),
    ('curiosity',       25, t("{s}'s feeling bored.")),
    ('playfulness',     30, t("{s}'s not feeling playful.")),
    ('affection',       25, t("{s} wants more attention.")),
    ('fulfillment',     25, t("{s}'s feeling unfulfilled.")),
    ('serenity',        25, t("{s} seems restless.")),
    ('sociability',     20, t("{s}'s been withdrawn.")),
    ('courage',         20, t("{s}'s been timid lately.")),
    ('loyalty',         20, t("{s} seems detached.")),
    ('mischievousness', 20, t("{s}'s been very subdued.")),
    ('maturity',        20, t("{s}'s been impulsive.")),
)


class PetInfoScene(Scene):
    SCENE_NAME = 'pet_info'

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._editing        = False
        self._kb_name        = OnScreenKeyboard(renderer, input, charset='full', max_len=12)
        self._portrait_poses = None
        self._head_sprite    = None
        self._eyes_sprite    = None
        self._lines          = []
        self._scroll         = 0
        self._max_scroll     = 0
        self._narrow_x       = 0   # x offset for lines beside the headshot
        self._narrow_lines   = 0   # how many leading lines use narrow x

    def load(self):
        super().load()

    def unload(self):
        super().unload()
        self._portrait_poses = None
        self._head_sprite    = None
        self._eyes_sprite    = None
        self._lines          = []

    def enter(self):
        self._editing = False
        self._scroll  = 0
        self._build_content()

    def exit(self):
        pass

    def update(self, dt):
        return None

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._editing:
            result = self._kb_name.handle_input()
            if result is not None:
                value = result.strip()
                if value:
                    self.context.pet_name = value
                self._editing = False
                self._build_content()
            return None

        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')
        if self.input.was_just_pressed('up') and self._scroll > 0:
            self._scroll -= 1
        elif self.input.was_just_pressed('down') and self._scroll < self._max_scroll:
            self._scroll += 1
        elif self.input.was_just_pressed('a') and self._scroll == self._max_scroll:
            self._kb_name.open('', self.context.pet_name or '')
            self._editing = True
        return None

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def draw(self):
        if self._editing:
            self._kb_name.draw()
            return

        r     = self.renderer
        total = len(self._lines)

        if self._head_sprite and self._scroll < self._narrow_lines:
            self._draw_headshot(-(self._scroll * 8))

        rows_remaining = max(0, self._narrow_lines - self._scroll)

        for i in range(_VISIBLE):
            line_idx = self._scroll + i
            if line_idx >= total:
                break
            y = i * 8
            x = self._narrow_x if i < rows_remaining else 3

            line           = self._lines[line_idx]
            is_change_name = (line_idx == total - 1)
            at_bottom      = (self._scroll == self._max_scroll)

            if is_change_name and at_bottom:
                r.draw_rect(0, y, 128, 8, filled=True, color=1)
                r.draw_text(line, x, y, 0)
            else:
                r.draw_text(line, x, y)

        if self._scroll > 0:
            r.draw_sprite(UP_ICON, 8, 8, 119, 0)
        if self._scroll < self._max_scroll:
            r.draw_sprite(DOWN_ICON, 8, 8, 119, 56)

    def _draw_headshot(self, y_offset=0):
        r    = self.renderer
        head = self._head_sprite
        eyes = self._eyes_sprite
        r.draw_sprite_obj(head, 0, y_offset)
        r.draw_sprite_obj(eyes,
                          head['eye_x'] - eyes['anchor_x'],
                          y_offset + head['eye_y'] - eyes['anchor_y'])

    # ------------------------------------------------------------------
    # Content building
    # ------------------------------------------------------------------

    def _build_content(self):
        from assets.character import POSES
        pp   = self._get_portrait_poses()
        ctx  = self.context
        seed = ctx.pet_seed or 0

        pname = pp[(seed >> 40) % len(pp)]
        parts = pname.split('.')
        pose  = POSES[parts[0]][parts[1]][parts[2]]

        self._head_sprite  = pose['head']
        self._eyes_sprite  = pose['eyes']
        head_w             = self._head_sprite['width']
        head_h             = self._head_sprite['height']
        self._narrow_lines = (head_h + 7) // 8   # ceiling: lines the headshot spans
        self._narrow_x     = head_w + 6           # 6 px gap between headshot and text
        narrow_cpl         = (128 - self._narrow_x - 3) // 8

        # Pronouns
        gender   = getattr(ctx, 'pet_gender', 'queen')
        she      = t('He')  if gender == 'tom' else t('She')
        she_l    = she.lower()
        Her      = t('His') if gender == 'tom' else t('Her')
        her      = Her.lower()
        g_noun   = t('tom') if gender == 'tom' else t('queen')

        # Intro paragraph 1: floated beside headshot
        name   = ctx.pet_name or '?'
        days   = ctx.environment.get('day_number', 0)
        sign   = getattr(ctx, 'star_sign', None) or '?'
        temper = self._temperament()
        intro1 = t('{name} is a {days} day old {gnoun}.').format(name=name, days=days, gnoun=g_noun)
        intro2 = t('{she} is a {temper} {sign}.').format(she=she, temper=temper.lower(), sign=sign.lower())
        intro1_lines = self._wrap_intro(intro1, narrow_cpl, _FULL_CPL, self._narrow_lines)
        intro2_lines = self._wrap(intro2, _FULL_CPL)

        # Body paragraphs
        dn      = self._display_name
        meal    = dn(ctx.fav_meal).lower()
        snack   = dn(ctx.fav_snack).lower()
        toy     = dn(ctx.fav_toy).lower()
        room    = dn(getattr(ctx, 'fav_location', None)).lower()
        weather = dn(getattr(ctx, 'fav_weather', None)).lower()

        body = []

        def _add(text):
            body.extend(self._wrap(text, _FULL_CPL))
            body.append('')

        _add(t('{Her} favorite meal is {meal}, and {her} favorite snack is {snack}.').format(Her=Her, meal=meal, her=her, snack=snack))
        _add(t('{She} loves to hang out in the {room}, and {she_l} really enjoys {weather} days.').format(She=she, room=room, she_l=she_l, weather=weather))
        _add(t('Of all the toys, {she_l} loves to play with the {toy} the most.').format(she_l=she_l, toy=toy))

        # Sickness
        sickness = getattr(ctx, 'sickness', 0.0)
        if sickness >= 7.0:
            _add(t('{she} feels very sick.').format(she=she))
        elif sickness >= 3.0:
            _add(t('{she} feels pretty sick.').format(she=she))
        elif sickness > 0.0:
            _add(t('{she} feels a little sick.').format(she=she))

        # Mood sentences: one per low stat; fallback to "feeling great"
        mood = []
        for stat, threshold, tmpl in _MOOD_CHECKS:
            val = getattr(ctx, stat, 100.0)
            if isinstance(val, (int, float)) and val < threshold:
                sentence = tmpl.replace('{s}', she).replace('{h}', her)
                if mood:
                    mood.append('')
                mood.extend(self._wrap(sentence, _FULL_CPL))
        if not mood:
            mood = self._wrap(t("It seems like {she_l}'s feeling pretty great at the moment!").format(she_l=she_l), _FULL_CPL)
        body.extend(mood)
        body.append('')

        # Meal variety: 4+ of the last 5 meals are the same
        recent = getattr(ctx, 'recent_meals', [])
        if len(recent) >= 4:
            counts = {}
            for m in recent:
                counts[m] = counts.get(m, 0) + 1
            dominant = 0
            for v in counts.values():
                if v > dominant:
                    dominant = v
            if dominant >= 4:
                _add(t('{she} wishes {she_l} had more variety in {her} meals.').format(she=she, she_l=she_l, her=her))

        # Familiar wifi location
        if getattr(ctx, 'in_familiar_location', False):
            _add(t('{she} feels at home here.').format(she=she))

        # Strip trailing blanks, then add a blank spacer before "Change Name"
        while body and body[-1] == '':
            body.pop()
        body.append('')
        body.append('')
        body.append(t('Change Name'))

        self._lines      = intro1_lines + [''] + intro2_lines + [''] + body
        self._max_scroll = max(0, len(self._lines) - _VISIBLE)

    def _wrap_intro(self, text, narrow_cpl, full_cpl, narrow_lines):
        """Wrap text, using narrow_cpl for the first narrow_lines lines then full_cpl."""
        words   = text.split(' ')
        lines   = []
        current = ''
        cpl     = narrow_cpl

        for word in words:
            if len(lines) >= narrow_lines:
                cpl = full_cpl
            test = current + (' ' if current else '') + word
            if len(test) <= cpl:
                current = test
            else:
                if current:
                    lines.append(current)
                    if len(lines) >= narrow_lines:
                        cpl = full_cpl
                # Hyphenate words that exceed the current column width
                while word and len(word) > cpl - 1:
                    lines.append(word[:cpl - 1] + '-')
                    word = word[cpl - 1:]
                    if len(lines) >= narrow_lines:
                        cpl = full_cpl
                current = word

        if current:
            lines.append(current)
        return lines

    def _wrap(self, text, cpl):
        """Word-wrap text to cpl chars, hyphenating words that exceed the width."""
        words   = text.split(' ')
        lines   = []
        current = ''
        for word in words:
            test = current + (' ' if current else '') + word
            if len(test) <= cpl:
                current = test
            else:
                if current:
                    lines.append(current)
                while word and len(word) > cpl - 1:
                    lines.append(word[:cpl - 1] + '-')
                    word = word[cpl - 1:]
                current = word
        lines.append(current)
        return lines

    # ------------------------------------------------------------------
    # Portrait helpers
    # ------------------------------------------------------------------

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

    # ------------------------------------------------------------------
    # Text helpers
    # ------------------------------------------------------------------

    def _temperament(self):
        ctx  = self.context
        vals = [getattr(ctx, k, 50) for k in _TRAIT_NAMES]
        return _TEMPERAMENT_LABELS[vals.index(max(vals))]

    @staticmethod
    def _display_name(key):
        if not key:
            return '?'
        if key in _TRANSLATED_NAMES:
            return _TRANSLATED_NAMES[key]
        if key in _DISPLAY_NAME_OVERRIDES:
            return _DISPLAY_NAME_OVERRIDES[key]
        s = key.replace('_', ' ')
        return ' '.join(w[0].upper() + w[1:] for w in s.split(' ') if w)
