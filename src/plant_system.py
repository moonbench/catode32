"""plant_system.py - Plant growth state machine and tick logic.

Plants are stored as dicts in context.plants.  Each dict has:
    id, type, scene, layer, x, pot, stage, age_hours,
    water_debt_hours, planted_day

Ticking is global: tick_plants(context) is called from MainScene.on_update
once per in-game hour and advances *all* plants, not just the active scene.
"""

# ---------------------------------------------------------------------------
# Type definitions
# ---------------------------------------------------------------------------

# Per-type thresholds (all in in-game hours).
# Time scale: game_minutes_per_second=1.0, so 1 in-game hour = 1 real minute,
#   1 real day = 1440 in-game hours.
# wilt:    hours of no water before wilting  (target: 1–2 real days = 1440–2880 h)
# death:   hours of no water before death    (wilt window = death - wilt)
# recover: max debt allowed for auto-recovery after watering while wilted
# stage_hours: time at each stage before advancing (target: 2–5 real days = 2880–7200 h)
#   index 0 = seedling→young, 1 = young→growing, 2 = growing→mature, 3 = mature→thriving
_PLANT_TYPES = {
    'cat_grass': {
        'wilt': 1440, 'death': 4320, 'recover': 240,  # 1 day / 3 days
        'stage_hours': (2880, 3600, 4320, 5040),       # 2, 2.5, 3, 3.5 real days
    },
    'fern': {
        'wilt': 2160, 'death': 5760, 'recover': 360,  # 1.5 days / 4 days
        'stage_hours': (3600, 4320, 5760, 7200),       # 2.5, 3, 4, 5 real days
    },
    'tulip': {
        'wilt': 1440, 'death': 4320, 'recover': 120,  # 1 day / 3 days
        'stage_hours': (4320, 5040, 6480, 7200),       # 3, 3.5, 4.5, 5 real days
        'dormant_in_winter': True,
    },
    'rose': {
        'wilt': 1440, 'death': 4320, 'recover': 120,  # 1 day / 3 days
        'stage_hours': (5040, 6480, 7200, 7200),       # 3.5, 4.5, 5, 5 real days
    },
    'sunflower': {
        'wilt': 2880, 'death': 5760, 'recover': 240,  # 2 days / 4 days
        'stage_hours': (2880, 4320, 5760, 7200, 7200), # 2, 3, 4, 5 real days (last unused)
        'indoor_max': 'growing',
    },
}

# Maximum growth stage permitted per pot type.
_POT_CAPS = {
    'small':   'young',
    'medium':  'growing',
    'large':   'mature',
    'planter': 'mature',
    'ground':  'thriving',
}

# Canonical growth order.  Wilted variants are handled with a suffix; 'dead'
# and 'dormant' are terminal/special and not in this sequence.
_STAGE_ORDER = ('seedling', 'young', 'growing', 'mature', 'thriving')
_STAGE_INDEX = {s: i for i, s in enumerate(_STAGE_ORDER)}

# Stages where time-based ticking is skipped entirely.
_INERT_STAGES = frozenset(('empty_pot', 'dead'))


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

def _base_stage(stage):
    """Return the base stage name, stripping the '_wilted' suffix if present."""
    if stage.endswith('_wilted'):
        return stage[:-7]
    return stage


def _is_wilted(stage):
    return stage.endswith('_wilted')


def _can_advance(plant, ptype):
    """Return True if the plant's pot and type allow advancing past its current stage."""
    base = _base_stage(plant['stage'])
    idx = _STAGE_INDEX.get(base, -1)
    if idx < 0 or idx + 1 >= len(_STAGE_ORDER):
        return False

    next_stage = _STAGE_ORDER[idx + 1]
    next_idx = idx + 1

    # Pot cap
    pot_cap = _POT_CAPS.get(plant['pot'], 'young')
    if next_idx > _STAGE_INDEX.get(pot_cap, 0):
        return False

    # Sunflower indoor cap
    indoor_max = ptype.get('indoor_max')
    if indoor_max and plant['scene'] != 'outside':
        if next_idx > _STAGE_INDEX.get(indoor_max, 0):
            return False

    return True


def _maturation_threshold(ptype, stage_idx):
    """Total age_hours required to leave stage_idx (cumulative sum)."""
    hours = ptype['stage_hours']
    total = 0
    for i in range(min(stage_idx + 1, len(hours))):
        total += hours[i]
    return total


# ---------------------------------------------------------------------------
# Per-plant tick (one in-game hour)
# ---------------------------------------------------------------------------

# Debt adjustment per hour for outdoor plants in wet weather.
# Applied after the normal +1, so net effect = delta + 1.
# Rain:  net -10 per hour (10 hours of debt cleared per rain hour).
# Storm: net -20 per hour (heavier downpour, twice the rain benefit).
_RAIN_DEBT_DELTA  = -11   # net -10
_STORM_DEBT_DELTA = -21   # net -20


def tick_plant(plant, season, weather='Clear'):
    """Advance one in-game hour for a single plant.

    Modifies the plant dict in place.  Returns True if the stage changed.
    weather is the current weather string from context.environment; used to
    reduce water debt for outdoor plants during Rain or Storm.
    """
    stage = plant['stage']

    if stage in _INERT_STAGES:
        return False

    ptype = _PLANT_TYPES.get(plant['type'], _PLANT_TYPES['fern'])

    # --- Outdoor winter handling ---
    if plant['scene'] == 'outside' and season == 'Winter':
        if ptype.get('dormant_in_winter') and stage not in ('dormant', 'dead', 'empty_pot'):
            plant['stage'] = 'dormant'
            return True
        # All outdoor plants pause in winter (no age/debt accumulation).
        return False

    # --- Dormant tulip waking in spring/summer/fall ---
    if stage == 'dormant':
        if season != 'Winter':
            plant['stage'] = 'seedling'
            plant['water_debt_hours'] = 0
            return True
        return False

    # --- Normal hour accumulation ---
    plant['age_hours'] = plant.get('age_hours', 0) + 1
    plant['water_debt_hours'] = plant.get('water_debt_hours', 0) + 1

    # --- Rain / storm watering for outdoor plants ---
    if plant['scene'] == 'outside':
        if weather == 'Rain':
            plant['water_debt_hours'] += _RAIN_DEBT_DELTA
        elif weather == 'Storm':
            plant['water_debt_hours'] += _STORM_DEBT_DELTA
        if plant['water_debt_hours'] < 0:
            plant['water_debt_hours'] = 0

    debt = plant['water_debt_hours']

    if _is_wilted(stage):
        # Watering recovery: debt was reset to 0 by player, so it's still low.
        if debt <= ptype['recover']:
            plant['stage'] = _base_stage(stage)
            return True
        # Death from prolonged neglect while wilted.
        if debt > ptype['death']:
            plant['stage'] = 'dead'
            return True

    else:
        # Wilt check.
        if debt > ptype['wilt']:
            plant['stage'] = stage + '_wilted'
            return True

        # Maturation check.
        base_idx = _STAGE_INDEX.get(stage, -1)
        if base_idx >= 0 and _can_advance(plant, ptype):
            threshold = _maturation_threshold(ptype, base_idx)
            if plant['age_hours'] >= threshold:
                plant['stage'] = _STAGE_ORDER[base_idx + 1]
                return True

    return False


# ---------------------------------------------------------------------------
# Global tick (called once per in-game hour from MainScene.on_update)
# ---------------------------------------------------------------------------

def tick_plants(context):
    """Advance all plants by the number of in-game hours elapsed since the last tick.

    Uses context.environment['day_number'] and ['time_hours'] to compute an
    absolute hour index.  context._last_plant_tick_hour is initialised on the
    first call after boot and never persisted — plants in unvisited scenes are
    caught up correctly because the hour index is absolute.
    """
    env = context.environment
    current_hour = env.get('day_number', 0) * 24 + env.get('time_hours', 0)

    last = getattr(context, '_last_plant_tick_hour', None)
    if last is None:
        # First call this session: anchor to now, no catch-up needed because
        # water_debt_hours already reflect elapsed real time via the save.
        context._last_plant_tick_hour = current_hour
        return

    hours_elapsed = current_hour - last
    if hours_elapsed <= 0:
        return

    season = env.get('season', 'Summer')
    weather = env.get('weather', 'Clear')
    plants = context.plants

    # Cap to 24 hours to avoid runaway ticks after very long pauses.
    for _ in range(min(hours_elapsed, 24)):
        for plant in plants:
            tick_plant(plant, season, weather)

    context._last_plant_tick_hour = current_hour


# ---------------------------------------------------------------------------
# Player actions
# ---------------------------------------------------------------------------

def water_plant(plant):
    """Reset water debt after the player waters a plant."""
    plant['water_debt_hours'] = 0


def plant_seed(context, scene, layer, x, y_snap, pot, plant_type):
    """Place a new seedling into context.plants.  Consumes one seed from inventory.

    Returns the new plant dict, or None if the seed was not available.
    """
    import random
    seeds = context.inventory.get('seeds', {})
    if seeds.get(plant_type, 0) <= 0:
        return None

    seeds[plant_type] -= 1

    pid = context.next_plant_id
    context.next_plant_id += 1

    plant = {
        'id': pid,
        'type': plant_type,
        'scene': scene,
        'layer': layer,
        'x': x,
        'y_snap': y_snap,
        'pot': pot,
        'stage': 'seedling',
        'age_hours': 0,
        'water_debt_hours': 0,
        'planted_day': context.environment.get('day_number', 0),
        'mirror': bool(random.getrandbits(1)),
    }
    context.plants.append(plant)
    return plant


def place_empty_pot(context, scene, layer, x, y_snap, pot_type):
    """Place an empty pot into context.plants.  Consumes one pot from inventory.

    Returns the new plant dict (stage='empty_pot'), or None if not available.
    """
    import random
    pots = context.inventory.get('pots', {})
    if pots.get(pot_type, 0) <= 0:
        return None

    pots[pot_type] -= 1

    pid = context.next_plant_id
    context.next_plant_id += 1

    plant = {
        'id': pid,
        'type': None,
        'scene': scene,
        'layer': layer,
        'x': x,
        'y_snap': y_snap,
        'pot': pot_type,
        'stage': 'empty_pot',
        'age_hours': 0,
        'water_debt_hours': 0,
        'planted_day': None,
        'mirror': bool(random.getrandbits(1)),
    }
    context.plants.append(plant)
    return plant


def remove_plant(context, plant_id):
    """Remove a plant from context.plants by id.

    If the plant is alive (not dead/empty_pot), returns the pot type to
    inventory so the player gets it back.  Dead plants are discarded.
    Returns True if found and removed.
    """
    for i, plant in enumerate(context.plants):
        if plant['id'] == plant_id:
            # Return pot to inventory only if plant is not dead.
            if plant['stage'] != 'dead' and plant['pot'] != 'ground':
                pots = context.inventory.setdefault('pots', {})
                pots[plant['pot']] = pots.get(plant['pot'], 0) + 1
            context.plants.pop(i)
            return True
    return False


def move_plant(context, plant_id, new_scene, new_layer, new_x):
    """Relocate a plant to a new scene/layer/x.  Stage and health are preserved."""
    for plant in context.plants:
        if plant['id'] == plant_id:
            plant['scene'] = new_scene
            plant['layer'] = new_layer
            plant['x'] = new_x
            return True
    return False


def repot_plant(context, plant_id, new_pot_type):
    """Move a plant into a larger pot from inventory.

    Returns True on success, False if the new pot type is not in inventory or
    is not larger than the current pot.
    """
    cap_order = ('small', 'medium', 'large', 'planter', 'ground')

    for plant in context.plants:
        if plant['id'] != plant_id:
            continue

        current_pot = plant['pot']
        if current_pot == 'ground' or new_pot_type == 'ground':
            return False  # Ground isn't a pot you can repot into/out of.

        current_rank = cap_order.index(current_pot) if current_pot in cap_order else 0
        new_rank = cap_order.index(new_pot_type) if new_pot_type in cap_order else 0
        if new_rank <= current_rank:
            return False  # Not an upgrade.

        pots = context.inventory.get('pots', {})
        if pots.get(new_pot_type, 0) <= 0:
            return False

        pots[new_pot_type] -= 1
        # Return old pot if it's a real pot type.
        if current_pot in ('small', 'medium', 'large', 'planter'):
            pots[current_pot] = pots.get(current_pot, 0) + 1

        plant['pot'] = new_pot_type
        return True

    return False


# ---------------------------------------------------------------------------
# Stat helpers (used by behavior completion bonuses)
# ---------------------------------------------------------------------------

def count_healthy_plants(context, scene_name):
    """Count non-wilted, non-dead plants in the given scene."""
    count = 0
    for p in context.plants:
        if p['scene'] != scene_name:
            continue
        s = p['stage']
        if s not in _INERT_STAGES and s != 'dormant' and not _is_wilted(s):
            count += 1
    return count


def count_dead_plants(context, scene_name):
    """Count dead plants in the given scene."""
    count = 0
    for p in context.plants:
        if p['scene'] == scene_name and p['stage'] == 'dead':
            count += 1
    return count


def get_plant_by_id(context, plant_id):
    """Return the plant dict with the given id, or None."""
    for p in context.plants:
        if p['id'] == plant_id:
            return p
    return None


def get_plants_for_scene(context, scene_name):
    """Return a list of all plant dicts for the given scene."""
    return [p for p in context.plants if p['scene'] == scene_name]


def inspect_lines(plant):
    """Return 1–3 short label strings for the Inspect submenu.

    Each string is at most 15 chars (fits 120px content area at 8px/char).
    Lines cover: identity (type + pot), stage/health, and watering need.
    """
    stage = plant.get('stage', 'empty_pot')
    plant_type = plant.get('type')
    pot = plant.get('pot', 'small')

    _TYPE_NAMES = {
        'cat_grass': 'Cat Grass', 'fern': 'Fern',
        'sunflower': 'Sunflower', 'rose': 'Rose', 'tulip': 'Tulip',
    }
    _POT_SHORT = {
        'small': 'sm', 'medium': 'md', 'large': 'lg',
        'planter': 'pl', 'ground': 'gr',
    }

    # Line 1: identity
    if stage == 'empty_pot' or plant_type is None:
        return ['Empty (' + _POT_SHORT.get(pot, pot) + ')']

    type_label = _TYPE_NAMES.get(plant_type, plant_type)
    line1 = type_label + ' (' + _POT_SHORT.get(pot, pot) + ')'

    # Terminal / special stages — no water line needed
    if stage == 'dead':
        return [line1, 'Stage: Dead']
    if stage == 'dormant':
        return [line1, 'Stage: Dormant']

    ptype = _PLANT_TYPES.get(plant_type, _PLANT_TYPES['fern'])
    debt = plant.get('water_debt_hours', 0)
    wilted = _is_wilted(stage)
    recovering = wilted and debt <= ptype['recover']

    if recovering:
        return [line1, 'Stage: Recovering']

    # Line 2: stage (use base stage name even when wilted)
    line2 = 'Stage: ' + stage_display_name(_base_stage(stage))

    # Line 3: watering need
    if wilted:
        remaining = ptype['death'] - debt
        water_label = 'Critical' if remaining <= ptype['death'] // 4 else 'Dry!'
    else:
        wilt = ptype['wilt']
        if debt == 0:
            water_label = 'Full'
        elif debt < wilt // 3:
            water_label = 'OK'
        elif debt < wilt * 2 // 3:
            water_label = 'Low'
        else:
            water_label = 'Urgent'

    return [line1, line2, 'Water: ' + water_label]


def stage_display_name(stage):
    """Human-readable label for a stage string."""
    _NAMES = {
        'empty_pot':      'Empty pot',
        'seedling':       'Seedling',
        'seedling_wilted': 'Wilting seedling',
        'young':          'Young',
        'young_wilted':   'Wilting',
        'growing':        'Growing',
        'growing_wilted': 'Wilting',
        'mature':         'Mature',
        'mature_wilted':  'Wilting',
        'thriving':       'Thriving',
        'thriving_wilted': 'Wilting',
        'dead':           'Dead',
        'dormant':        'Dormant',
    }
    return _NAMES.get(stage, stage)
