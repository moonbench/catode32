"""Lazy-loaded reset helpers for GameContext.

This module is imported only when a reset is requested, then immediately
unloaded via sys.modules.pop() so its bytecode and data don't occupy RAM
during normal gameplay.
"""


def _generate_seed():
    """Generate a 64-bit seed using hardware RNG (ESP32) or os.urandom fallback."""
    try:
        import os
        b = os.urandom(8)
        seed = 0
        for byte in b:
            seed = (seed << 8) | byte
        return seed
    except Exception:
        import random
        return random.getrandbits(32) | (random.getrandbits(32) << 32)


def _xorshift32(x):
    x ^= (x << 13) & 0xFFFFFFFF
    x ^= (x >> 17)
    x ^= (x << 5) & 0xFFFFFFFF
    return x & 0xFFFFFFFF


_PERSONALITY_TRAITS = ('courage', 'loyalty', 'mischievousness', 'curiosity', 'sociability')
_TRAIT_MAGNITUDE = 10  # max offset in either direction


def _derive_trait_offsets(seed):
    """Derive balanced personality offsets from a 64-bit seed.

    Generates an offset for each personality trait in [-_TRAIT_MAGNITUDE, +_TRAIT_MAGNITUDE].
    The offsets are mean-centered so no seed produces a uniformly happier or sadder cat.
    """
    state = (seed ^ (seed >> 32)) & 0xFFFFFFFF or 1
    span = 2 * _TRAIT_MAGNITUDE + 1  # 0 to 2*MAGNITUDE inclusive
    raw = []
    for _ in range(len(_PERSONALITY_TRAITS)):
        state = _xorshift32(state)
        raw.append(state % span)
    mean = sum(raw) // len(raw)
    return [v - mean for v in raw]


def _make_starter_plants():
    """Return the initial set of plants that replace the old static scene decorations.

    These mirror the developer-placed planters that were hard-coded in each
    scene's setup_scene() method.  They start at 'young' stage with zero water
    debt so the world feels alive without immediately demanding player attention.
    """
    _id = [0]

    def _p(scene, layer, x, y_snap, pot, plant_type, stage='young', age_hours=80, mirror=False):
        plant = {
            'id': _id[0],
            'type': plant_type,
            'scene': scene,
            'layer': layer,
            'x': x,
            'y_snap': y_snap,
            'pot': pot,
            'stage': stage,
            'age_hours': age_hours,
            'water_debt_hours': 0,
            'fertilizer': 0.0,
            'planted_day': 0,
            'mirror': mirror,
        }
        _id[0] += 1
        return plant

    return [
        _p('inside', 'midground', 110, 29, 'small', 'rose', stage='growing', age_hours=200),
        _p('inside', 'midground', 130, 29, 'planter', 'freesia', stage='young', age_hours=400, mirror=True),
        _p('inside', 'foreground', 15, 15, 'small', 'freesia', stage='young', age_hours=96),
        _p('inside', 'foreground', 140, 63, 'medium', 'cat_grass', stage='thriving', age_hours=80),

        _p('kitchen', 'foreground', 10, 63, 'small', 'cat_grass', stage='growing', age_hours=180, mirror=True),
        _p('kitchen', 'midground', 126, 24, 'medium', 'cat_grass', stage='mature', age_hours=160),
        _p('kitchen', 'midground', 62, 24, 'small', 'rose', stage='growing', age_hours=85, mirror=True),
        _p('kitchen', 'midground', 45, 24, 'small', 'cat_grass', stage='growing', age_hours=170),

        _p('outside', 'foreground', 10, 63, 'small', 'cat_grass', stage='growing', age_hours=160),
        _p('outside', 'foreground', 180, 63, 'small', 'freesia', stage='growing', age_hours=155),
        _p('outside', 'foreground', 210, 63, 'ground', 'cat_grass', stage='mature', age_hours=145, mirror=True),
        _p('outside', 'midground', 154, 61, 'ground', 'cat_grass', stage='growing', age_hours=150),
        _p('outside', 'midground', 130, 61, 'ground', 'sunflower', stage='mature', age_hours=360, mirror=True),
        _p('outside', 'midground', 40, 61, 'ground', 'cat_grass', stage='thriving', age_hours=150),
        _p('outside', 'background', 110, 56, 'ground', 'cat_grass', stage='thriving', age_hours=320),
        _p('outside', 'background', 55, 56, 'ground', 'cat_grass', stage='mature', age_hours=320),

        _p('treehouse', 'foreground', 15, 63, 'small', 'rose', stage='young', age_hours=72),
        _p('treehouse', 'foreground', 200, 63, 'medium', 'cat_grass', stage='growing', age_hours=155, mirror=True),
        _p('treehouse', 'midground', 30, 59, 'small', 'cat_grass', stage='growing', age_hours=150),
        _p('treehouse', 'midground', 120, 59, 'medium', 'freesia', stage='thriving', age_hours=88, mirror=True),
    ]


def reset_plants(ctx):
    """Restore all plants to the default starter set."""
    ctx.plants = _make_starter_plants()
    ctx.next_plant_id = len(ctx.plants)
    ctx._last_plant_tick_hour = None


def do_reset(ctx, delete_save):
    """Reset all stats on ctx to defaults. Optionally delete the save file."""
    # Unique 64-bit identity for this pet (survives between saves)
    ctx.pet_seed = _generate_seed()

    # Meta stat (computed from other stats)
    ctx.health = 50

    # Rapidly changing stats (change on a daily basis)
    ctx.fullness = 50          # Inverse of hunger. Feed to maintain.
    ctx.energy = 50            # How rested the pet is
    ctx.comfort = 50           # Physical comfort. Temperature, environment, etc...
    ctx.playfulness = 50       # Mood to play
    ctx.focus = 50             # Ability to concentrate on tasks/training

    # Slower changing stats (change on more of a weekly basis)
    ctx.fulfillment = 50       # Feeling like the pet has purpose and things to do
    ctx.cleanliness = 50       # How clean the pet and its environment are
    ctx.intelligence = 50      # Problem-solving, learning new skills/tricks
    ctx.maturity = 50          # Behavioral sophistication
    ctx.affection = 50         # How much the pet feels loved

    # Even slower changing stats (change on more of a monthly basis)
    ctx.fitness = 50           # Athleticism
    ctx.serenity = 50          # Inner peace. Makes them less likely to be stressed

    # Slowest changing stats (basically traits with little or no change).
    # Offset by a balanced, seed-derived personality so each pet feels distinct.
    _offsets = _derive_trait_offsets(ctx.pet_seed)
    ctx.courage =          50 + _offsets[0]
    ctx.loyalty =          50 + _offsets[1]
    ctx.mischievousness =  50 + _offsets[2]
    ctx.curiosity =        50 + _offsets[3]
    ctx.sociability =      50 + _offsets[4]

    # Coins (earned from minigames and hunting, spent in the store)
    ctx.coins = 50

    # Quantities of food purchased from the store (uses per type)
    ctx.food_stock = {
        "chicken": 0, "salmon": 0, "tuna": 0, "shrimp": 0, "mackerel": 0, "kibble": 5,
        "chew_stick": 0, "nugget": 3, "puree": 0, "milk": 0, "fish_bite": 0,
    }

    # Inventory of owned items
    ctx.inventory = {
        "toys": [
            {"name": "Feather", "variant": "toy"},
        ],
        "pots":       {"small": 0, "medium": 0, "large": 0, "planter": 0},
        "seeds":      {"cat_grass": 0, "sunflower": 0, "rose": 0, "tulip": 0},
        "tools":      {"watering_can": False, "spade": False},
        "fertilizer": 0,
    }

    # Living plants placed by the player (and the developer-seeded starters).
    # Each entry is a dict; see plant_system.py for the full schema.
    reset_plants(ctx)

    # Non-persisted: pending plant move across scenes.
    # Set by the Tend→"Move to" action; consumed by the destination scene on enter.
    ctx.pending_gardening_move = None

    # Minigame high scores
    ctx.zoomies_high_score = 0
    ctx.maze_best_time = 0  # Best time in seconds (0 = not played)
    ctx.snake_high_score = 0
    ctx.memory_best_score = -1  # Fewest mismatches (-1 = not yet played)
    ctx.hanjie_best_time = -1   # Fastest solve in seconds (-1 = not yet played)

    # For storing time/weather/season/moon-phase type data
    ctx.environment = {}

    # Debug: time scale multiplier (1.0 = normal, 2.0 = 2x speed, 0.0 = paused)
    ctx.time_speed = 1.0

    # Scene bounds for character movement (world coordinates, set by each scene on load)
    ctx.scene_x_min = 10
    ctx.scene_x_max = 118

    # Time of last save in ticks_ms; None = never saved this session
    ctx.last_save_time = None

    # WiFi location tracking (lists persisted; flag is not)
    ctx.wifi_familiar = []          # up to 16 well-known APs (persisted)
    ctx.wifi_recent = []            # up to 8 candidate APs (persisted)
    ctx.in_familiar_location = True   # updated each scan; defaults True when WiFi disabled

    # Recent completed behavior names for loop prevention (most recent first, not persisted)
    ctx.recent_behaviors = []

    # Name of the most recently started behavior (not persisted, used to restore on scene re-entry)
    ctx.current_behavior_name = None

    # Last active "main" scene (inside/outside/etc) - used by secondary scenes to return home
    ctx.last_main_scene = 'inside'

    # Requested scene change from a behavior (e.g. go_to on arrival). Cleared by scene_manager.
    ctx.pending_scene = None

    # ESP-NOW manager; injected by Game.__init__ when WIFI_ENABLED. None otherwise.
    ctx.espnow = None

    # Pet display name. Derived from MAC on first social scene entry; None until then.
    ctx.pet_name = None

    # Active visit state. None when not visiting, otherwise:
    #   {'peer_mac': bytes, 'peer_name': str, 'role': str,
    #    'greeted': bool, 'play_time': float}
    ctx.visit = None

    # Friends: mac_hex_str -> {'n': name, 't': total_seconds, 'c': visit_count}
    ctx.friends = {}

    if delete_save:
        try:
            import uos
            uos.remove('/save.json')
            print("[Context] Save file deleted")
        except:
            pass
    print("[Context] Reset to defaults")
