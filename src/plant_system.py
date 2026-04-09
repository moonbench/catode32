"""plant_system.py - Plant growth state machine and tick logic.

Plants are stored as dicts in context.plants.  Each dict has:
    id, type, scene, layer, x, pot, stage, age_hours,
    water_debt_hours, planted_day, fertilizer

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
    'freesia': {
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
        # Annuals die naturally: wilt after 19 real days, dead 2 days later.
        # Outdoor: reaches thriving at ~14d; spends ~5d there before end of life.
        'max_age_hours': 27360,        # 19 real days
        'max_age_death_window': 2880,  # 2 real days in wilted state before dead
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

# Fertilizer level thresholds and decay.
# Each application adds 100 (capped at 200). Decays 0.015/hour.
# At 100→20 takes ~5333 hours (~3.7 real days) — ideal re-apply window.
# Applying daily (1440h) leaves ~78 remaining before top-up → 178 → Over.
_FERT_DECAY = 0.015
_FERT_NO_MAX  =   5.0   # ≤5  → No
_FERT_LOW_MAX =  20.0   # ≤20 → Low
_FERT_OK_MAX  = 120.0   # ≤120 → OK;  >120 → Over
_FERT_ADD     = 100.0
_FERT_CAP     = 200.0


def _fert_ok(fert):
    """Return True if fertilizer level is in the OK range (enables thriving)."""
    return _FERT_LOW_MAX < fert <= _FERT_OK_MAX


def _fert_label(fert):
    if fert <= _FERT_NO_MAX:
        return 'No'
    elif fert <= _FERT_LOW_MAX:
        return 'Low'
    elif fert <= _FERT_OK_MAX:
        return 'OK'
    return 'Over'


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

    # Fertilizer gate: mature → thriving requires OK fertilizer level
    if next_stage == 'thriving':
        if not _fert_ok(plant.get('fertilizer', 0.0)):
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

    ptype = _PLANT_TYPES.get(plant['type'], _PLANT_TYPES['cat_grass'])

    # --- Outdoor winter handling ---
    if plant['scene'] == 'outside' and season == 'Winter':
        if ptype.get('dormant_in_winter') and stage not in ('dormant', 'dead', 'empty_pot'):
            plant['stage'] = 'dormant'
            return True
        # All outdoor plants pause in winter (no age/debt accumulation).
        return False

    # --- Dormant freesia waking in spring/summer/fall ---
    if stage == 'dormant':
        if season != 'Winter':
            plant['stage'] = 'seedling'
            plant['water_debt_hours'] = 0
            return True
        return False

    # --- Normal hour accumulation ---
    plant['age_hours'] = plant.get('age_hours', 0) + 1
    plant['water_debt_hours'] = plant.get('water_debt_hours', 0) + 1
    plant['fertilizer'] = max(0.0, plant.get('fertilizer', 0.0) - _FERT_DECAY)

    # --- Rain / storm watering for outdoor plants ---
    if plant['scene'] == 'outside':
        if weather == 'Rain':
            plant['water_debt_hours'] += _RAIN_DEBT_DELTA
        elif weather == 'Storm':
            plant['water_debt_hours'] += _STORM_DEBT_DELTA
        if plant['water_debt_hours'] < 0:
            plant['water_debt_hours'] = 0

    # --- Age-based lifecycle (annual plants, e.g. sunflower) ---
    max_age = ptype.get('max_age_hours')
    if max_age:
        age = plant['age_hours']
        death_window = ptype.get('max_age_death_window', 2880)
        if age >= max_age + death_window:
            plant['stage'] = 'dead'
            return True
        if age >= max_age:
            if not _is_wilted(stage):
                # Begin end-of-life wilt; mark so watering won't revive it.
                plant['stage'] = stage + '_wilted'
                plant['aged'] = True
                return True
            # Already age-wilted: hold until the death window above fires.
            return False

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
        # Thriving knockback: if fertilizer leaves OK range, drop back to mature.
        if stage == 'thriving' and not _fert_ok(plant.get('fertilizer', 0.0)):
            plant['stage'] = 'mature'
            return True

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


def fertilize_plant(plant):
    """Apply one unit of fertilizer to a plant (adds 100, capped at 200)."""
    plant['fertilizer'] = min(_FERT_CAP, plant.get('fertilizer', 0.0) + _FERT_ADD)


def plant_seed(context, pot_id, plant_type):
    """Plant a seed into an existing empty pot, identified by pot_id.

    Modifies the pot entry in place (stage empty_pot → seedling) so the same
    dict is reused and no duplicate entry is created.  Consumes one seed from
    inventory.  Returns the updated plant dict, or None if the seed was not
    available or the pot was not found / not empty.
    """
    import random
    seeds = context.inventory.get('seeds', {})
    if seeds.get(plant_type, 0) <= 0:
        return None

    for plant in context.plants:
        if plant['id'] == pot_id and plant['stage'] == 'empty_pot':
            seeds[plant_type] -= 1
            plant['type'] = plant_type
            plant['stage'] = 'seedling'
            plant['age_hours'] = 0
            plant['water_debt_hours'] = 0
            plant['fertilizer'] = 0.0
            plant['planted_day'] = context.environment.get('day_number', 0)
            plant['mirror'] = bool(random.getrandbits(1))
            return plant

    return None


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
        'fertilizer': 0.0,
        'planted_day': None,
        'mirror': bool(random.getrandbits(1)),
    }
    context.plants.append(plant)
    return plant


def plant_in_ground(context, scene, layer, x, y_snap, plant_type):
    """Plant a seed directly in the ground (no pot). Consumes one seed from inventory.

    Returns the new plant dict (stage='seedling'), or None if seed not available.
    Unlike place_empty_pot, this skips the empty-pot stage and goes straight to seedling.
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
        'pot': 'ground',
        'stage': 'seedling',
        'age_hours': 0,
        'water_debt_hours': 0,
        'fertilizer': 0.0,
        'planted_day': context.environment.get('day_number', 0),
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


def move_plant(context, plant_id, new_scene, new_layer, new_x, new_y_snap=None):
    """Relocate a plant to a new scene/layer/x (and optionally y_snap).

    Stage and health are preserved.  Pass new_y_snap when the surface changes
    (different shelf height or a cross-scene move).
    """
    for plant in context.plants:
        if plant['id'] == plant_id:
            plant['scene'] = new_scene
            plant['layer'] = new_layer
            plant['x'] = new_x
            if new_y_snap is not None:
                plant['y_snap'] = new_y_snap
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


_INSPECT_TYPE_NAMES = {
    'cat_grass': 'Cat Grass',
    'sunflower': 'Sunflower', 'rose': 'Rose', 'freesia': 'Freesia',
}
_INSPECT_POT_LABELS = {
    'small': 'Small', 'medium': 'Medium', 'large': 'Large',
    'planter': 'Planter', 'ground': 'Ground',
}
_STAGE_DISPLAY_NAMES = {
    'empty_pot':       'Empty pot',
    'seedling':        'Seedling',
    'seedling_wilted': 'Wilting seedling',
    'young':           'Young',
    'young_wilted':    'Wilting',
    'growing':         'Growing',
    'growing_wilted':  'Wilting',
    'mature':          'Mature',
    'mature_wilted':   'Wilting',
    'thriving':        'Thriving',
    'thriving_wilted': 'Wilting',
    'dead':            'Dead',
    'dormant':         'Dormant',
}


def inspect_lines(plant):
    """Return label strings for the Inspect submenu.

    Each string is at most 15 chars (fits 120px content area at 8px/char).
    Lines: type name, pot, stage/health, watering need (or lifespan notice).
    """
    stage = plant.get('stage', 'empty_pot')
    plant_type = plant.get('type')
    pot = plant.get('pot', 'small')

    pot_line = 'Pot: ' + _INSPECT_POT_LABELS.get(pot, pot)

    # Empty pot: just identity, no stage/water lines
    if stage == 'empty_pot' or plant_type is None:
        return ['Empty pot', pot_line]

    type_label = _INSPECT_TYPE_NAMES.get(plant_type, plant_type)

    # Terminal / special stages
    if stage == 'dead':
        return [type_label, pot_line, 'Stage: Dead']
    if stage == 'dormant':
        return [type_label, pot_line, 'Stage: Dormant']

    ptype = _PLANT_TYPES.get(plant_type, _PLANT_TYPES['cat_grass'])
    debt = plant.get('water_debt_hours', 0)
    aged = plant.get('aged', False)
    wilted = _is_wilted(stage)
    recovering = wilted and not aged and debt <= ptype['recover']

    if recovering:
        return [type_label, pot_line, 'Stage: Recovering']

    stage_line = 'Stage: ' + stage_display_name(_base_stage(stage))

    # Status line: lifespan notice takes priority over water for aged plants
    if aged:
        status = 'Natural lifespan'
    elif wilted:
        remaining = ptype['death'] - debt
        status = 'Water: Critical' if remaining <= ptype['death'] // 4 else 'Water: Dry!'
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
        status = 'Water: ' + water_label

    fert_line = 'Fert: ' + _fert_label(plant.get('fertilizer', 0.0))
    return [type_label, pot_line, stage_line, status, fert_line]


def stage_display_name(stage):
    """Human-readable label for a stage string."""
    return _STAGE_DISPLAY_NAMES.get(stage, stage)
