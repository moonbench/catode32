_SAVE_PATH = '/save.json'


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
            'planted_day': 0,
            'mirror': mirror,
        }
        _id[0] += 1
        return plant

    return [
        _p('inside', 'midground', 110, 29, 'small', 'rose', stage='growing', age_hours=200),
        _p('inside', 'midground', 130, 29, 'planter', 'tulip', stage='young', age_hours=400, mirror=True),
        _p('inside', 'foreground', 15, 15, 'small', 'tulip', stage='young', age_hours=96),
        _p('inside', 'foreground', 140, 63, 'medium', 'cat_grass', stage='thriving', age_hours=80),

        _p('kitchen', 'foreground', 10, 63, 'small', 'cat_grass', stage='growing', age_hours=180, mirror=True),
        _p('kitchen', 'midground', 130, 24, 'medium', 'cat_grass', stage='mature', age_hours=160),
        _p('kitchen', 'midground', 62, 24, 'small', 'rose', stage='growing', age_hours=85, mirror=True),
        _p('kitchen', 'midground', 45, 24, 'small', 'cat_grass', stage='growing', age_hours=170),

        _p('outside', 'foreground', 10, 63, 'small', 'cat_grass', stage='growing', age_hours=160),
        _p('outside', 'foreground', 180, 63, 'small', 'tulip', stage='growing', age_hours=155),
        _p('outside', 'foreground', 210, 63, 'ground', 'cat_grass', stage='mature', age_hours=145, mirror=True),
        _p('outside', 'midground', 154, 61, 'ground', 'cat_grass', stage='growing', age_hours=150),
        _p('outside', 'midground', 130, 61, 'ground', 'sunflower', stage='mature', age_hours=360, mirror=True),
        _p('outside', 'midground', 40, 61, 'ground', 'cat_grass', stage='thriving', age_hours=150),
        _p('outside', 'background', 110, 56, 'ground', 'cat_grass', stage='thriving', age_hours=320),
        _p('outside', 'background', 55, 56, 'ground', 'cat_grass', stage='mature', age_hours=320),

        _p('treehouse', 'foreground', 15, 63, 'small', 'rose', stage='young', age_hours=72),
        _p('treehouse', 'foreground', 200, 63, 'medium', 'cat_grass', stage='growing', age_hours=155, mirror=True),
        _p('treehouse', 'midground', 30, 59, 'small', 'cat_grass', stage='growing', age_hours=150),
        _p('treehouse', 'midground', 120, 59, 'medium', 'tulip', stage='thriving', age_hours=88, mirror=True),
    ]


_STAT_KEYS = (
    'fullness', 'energy', 'comfort', 'playfulness', 'focus',
    'fulfillment', 'cleanliness', 'curiosity', 'sociability',
    'intelligence', 'maturity', 'affection',
    'fitness', 'serenity',
    'courage', 'loyalty', 'mischievousness',
    'zoomies_high_score', 'maze_best_time', 'snake_high_score', 'memory_best_score', 'hanjie_best_time', 'time_speed',
    'coins',
)


class GameContext:
    def __init__(self):
        self.reset(delete_save=False)

    @property
    def meteor_shower_happening(self):
        """True when a meteor shower is currently active."""
        return self.environment.get('meteor_shower_timer', 0.0) > 0

    @property
    def scene_plant_health(self):
        """Aggregate plant health score for the current scene.

        Iterates over all plants in last_main_scene and returns a numeric score:
          +2 per thriving plant, +1 per other healthy plant (young/growing/mature),
          -1 per withering plant, -2 per dead plant.
        A positive score means the surroundings feel lush and well-tended;
        a negative score means the environment is neglected.
        """
        scene = self.last_main_scene
        score = 0
        for p in self.plants:
            if p.get('scene') != scene:
                continue
            stage = p.get('stage', '')
            if stage == 'thriving':
                score += 2
            elif stage == 'dead':
                score -= 2
            elif stage == 'withering':
                score -= 1
            else:
                score += 1
        return score

    def apply_stat_changes(self, changes):
        """Apply a dict of stat changes with asymptotic damping near extremes.

        Uses the same curve as BaseBehavior.apply_completion_bonus so minigame
        rewards feel consistent with behavior rewards. Stats near their ceiling
        resist further increases; stats near the floor resist further decreases.

        Args:
            changes: Dict mapping stat name to delta value (positive or negative).
        """
        EXP = 0.7
        for stat, delta in changes.items():
            if delta == 0:
                continue
            current = getattr(self, stat, None)
            if current is None or not isinstance(current, (int, float)):
                continue
            current = float(current)
            if delta > 0:
                room = (100.0 - current) / 100.0
                delta *= room ** EXP
            else:
                room = current / 100.0
                delta *= room ** EXP
            new_value = max(0.0, min(100.0, current + delta))
            color = "\033[32m" if delta > 0 else "\033[31m"
            print(f"[\033[36mStat\033[0m] {stat}: {current:.1f} {color}{delta:+.2f}\033[0m -> {new_value:.1f}")
            setattr(self, stat, new_value)
        self.recompute_health()

    def recompute_health(self):
        """Recompute health as a weighted average of contributing stats.

        Called after each behavior completes and applies its stat changes.
        Health is never modified directly — it is always derived.
        """
        raw = (
            0.25 * self.fullness +
            0.20 * self.fitness +
            0.20 * self.energy +
            0.15 * self.cleanliness +
            0.05 * self.comfort +
            0.05 * self.affection +
            0.025 * self.fulfillment +
            0.025 * self.focus +
            0.025 * self.intelligence +
            0.025 * self.playfulness
        )
        self.health = max(0.0, min(100.0, raw))

    def debug_print_stats(self):
        print("Stats:")
        print("Fullness:     %6.4f, Energy:       %6.4f, Comfort:         %6.4f" % (self.fullness, self.energy, self.comfort))
        print("Playfulness:  %6.4f, Focus:        %6.4f" % (self.playfulness, self.focus))
        print("----------------------------------------------------------------")
        print("Health:       %6.4f, Fulfillment:  %6.4f, Cleanliness:     %6.4f" % (self.health, self.fulfillment, self.cleanliness))
        print("Curiosity:    %6.4f, Sociability:  %6.4f" % (self.curiosity, self.sociability))
        print("Intelligence: %6.4f, Maturity:     %6.4f, Affection:       %6.4f" % (self.intelligence, self.maturity, self.affection))
        print("----------------------------------------------------------------")
        print("Fitness:      %6.4f, Serenity:     %6.4f" % (self.fitness, self.serenity))
        print("----------------------------------------------------------------")
        print("Courage:      %6.4f, Loyalty:      %6.4f, Mischievousness: %6.4f" % (self.courage, self.loyalty, self.mischievousness))
        print("----------------------------------------------------------------")

    def _write_to_flash(self):
        """Write stats to flash without rebooting. Returns True on success."""
        import ujson
        import time
        data = {'v': 1, 'env': self.environment, 'food_stock': self.food_stock,
                'toys': self.inventory["toys"],
                'pots': self.inventory["pots"],
                'seeds': self.inventory["seeds"],
                'tools': self.inventory["tools"],
                'plants': self.plants,
                'next_plant_id': self.next_plant_id,
                'pet_seed': self.pet_seed,
                'wifi_familiar': self.wifi_familiar, 'wifi_recent': self.wifi_recent,
                'pet_name': self.pet_name, 'friends': self.friends}
        for key in _STAT_KEYS:
            data[key] = getattr(self, key)
        try:
            with open(_SAVE_PATH, 'w') as f:
                ujson.dump(data, f)
            import uos
            uos.sync()
            self.last_save_time = time.ticks_ms()
            return True
        except Exception as e:
            print("[Context] Save failed: " + str(e))
            return False

    def reset_plants(self):
        """Restore all plants to the default starter set."""
        self.plants = _make_starter_plants()
        self.next_plant_id = len(self.plants)
        self._last_plant_tick_hour = None

    def save(self):
        """Write stats to flash then reboot."""
        import sys
        if not self._write_to_flash():
            return
        if '/remote' in sys.path:
            # Running under mpremote mount (dev mode) — soft reset would
            # kill the mount and crash mpremote, so skip it.
            print("[Context] Saved to " + _SAVE_PATH + " (dev mode, no reboot)")
        else:
            print("[Context] Saved to " + _SAVE_PATH + ", rebooting...")
            import machine
            machine.soft_reset()

    def load(self):
        """Load stats from flash storage. Returns True if successful."""
        import ujson
        try:
            with open(_SAVE_PATH, 'r') as f:
                data = ujson.load(f)
            for key in _STAT_KEYS:
                if key in data:
                    setattr(self, key, data[key])
            self.pet_seed = data.get('pet_seed') or _generate_seed()
            self.pet_name = data.get('pet_name')
            self.environment = data.get('env', {})
            if 'food_stock' in data:
                self.food_stock.update(data['food_stock'])
            if 'toys' in data:
                self.inventory['toys'] = data['toys']
            if 'pots' in data:
                self.inventory['pots'].update(data['pots'])
            if 'seeds' in data:
                self.inventory['seeds'].update(data['seeds'])
            if 'tools' in data:
                self.inventory['tools'].update(data['tools'])
            # Plants: if absent from save (old save file), keep the starter plants
            # that reset() already populated.
            if 'plants' in data:
                _LAYER_MIGRATE = {'fg': 'foreground', 'mg': 'midground', 'bg': 'background'}
                for p in data['plants']:
                    p['layer'] = _LAYER_MIGRATE.get(p.get('layer'), p.get('layer'))
                self.plants = data['plants']
            self.next_plant_id = data.get('next_plant_id', len(self.plants))
            self.wifi_familiar = data.get('wifi_familiar', [])
            self.wifi_recent   = data.get('wifi_recent',   [])
            self.friends       = data.get('friends',       {})
            self.recompute_health()
            import time
            self.last_save_time = time.ticks_ms()
            print("[Context] Loaded from " + _SAVE_PATH)
            return True
        except Exception as e:
            print("[Context] Load skipped: " + str(e))
            return False

    def reset(self, delete_save=True):
        """Reset all stats to defaults. Optionally delete the save file."""
        # Unique 64-bit identity for this pet (survives between saves)
        self.pet_seed = _generate_seed()

        # Meta stat (computed from other stats)
        self.health = 50

        # Rapidly changing stats (change on a daily basis)
        self.fullness = 50          # Inverse of hunger. Feed to maintain.
        self.energy = 50            # How rested the pet is
        self.comfort = 50           # Physical comfort. Temperature, environment, etc...
        self.playfulness = 50       # Mood to play
        self.focus = 50             # Ability to concentrate on tasks/training

        # Slower changing stats (change on more of a weekly basis)
        self.fulfillment = 50       # Feeling like the pet has purpose and things to do
        self.cleanliness = 50       # How clean the pet and its environment are
        self.intelligence = 50      # Problem-solving, learning new skills/tricks
        self.maturity = 50          # Behavioral sophistication
        self.affection = 50         # How much the pet feels loved

        # Even slower changing stats (change on more of a monthly basis)
        self.fitness = 50           # Athleticism
        self.serenity = 50          # Inner peace. Makes them less likely to be stressed

        # Slowest changing stats (basically traits with little or no change).
        # Offset by a balanced, seed-derived personality so each pet feels distinct.
        _offsets = _derive_trait_offsets(self.pet_seed)
        self.courage =          50 + _offsets[0]
        self.loyalty =          50 + _offsets[1]
        self.mischievousness =  50 + _offsets[2]
        self.curiosity =        50 + _offsets[3]
        self.sociability =      50 + _offsets[4]

        # Coins (earned from minigames and hunting, spent in the store)
        self.coins = 50

        # Quantities of food purchased from the store (uses per type)
        self.food_stock = {
            "chicken": 0, "salmon": 0, "tuna": 0, "shrimp": 0, "mackerel": 0, "kibble": 5,
            "chew_stick": 0, "nugget": 3, "cream": 0, "milk": 0, "fish_bite": 0,
        }

        # Inventory of owned items
        self.inventory = {
            "toys": [
                {"name": "Feather", "variant": "toy"},
            ],
            "pots":  {"small": 0, "medium": 0, "large": 0, "planter": 0},
            "seeds": {"cat_grass": 0, "sunflower": 0, "rose": 0, "tulip": 0},
            "tools": {"watering_can": False, "spade": False},
        }

        # Living plants placed by the player (and the developer-seeded starters).
        # Each entry is a dict; see plant_system.py for the full schema.
        self.plants = _make_starter_plants()
        self.next_plant_id = len(self.plants)

        # Non-persisted: absolute in-game hour of the last plant tick.
        # Initialised lazily on the first tick_plants() call each session.
        self._last_plant_tick_hour = None

        # Non-persisted: pending plant move across scenes.
        # Set by the Tend→"Move to" action; consumed by the destination scene on enter.
        self.pending_gardening_move = None

        # Minigame high scores
        self.zoomies_high_score = 0
        self.maze_best_time = 0  # Best time in seconds (0 = not played)
        self.snake_high_score = 0
        self.memory_best_score = -1  # Fewest mismatches (-1 = not yet played)
        self.hanjie_best_time = -1   # Fastest solve in seconds (-1 = not yet played)

        # For storing time/weather/season/moon-phase type data
        self.environment = {}

        # Debug: time scale multiplier (1.0 = normal, 2.0 = 2x speed, 0.0 = paused)
        self.time_speed = 1.0

        # Scene bounds for character movement (world coordinates, set by each scene on load)
        self.scene_x_min = 10
        self.scene_x_max = 118

        # Time of last save in ticks_ms; None = never saved this session
        self.last_save_time = None

        # WiFi location tracking (lists persisted; flag is not)
        self.wifi_familiar = []          # up to 16 well-known APs (persisted)
        self.wifi_recent = []            # up to 8 candidate APs (persisted)
        self.in_familiar_location = True   # updated each scan; defaults True when WiFi disabled

        # Recent completed behavior names for loop prevention (most recent first, not persisted)
        self.recent_behaviors = []

        # Name of the most recently started behavior (not persisted, used to restore on scene re-entry)
        self.current_behavior_name = None

        # Last active "main" scene (inside/outside/etc) - used by secondary scenes to return home
        self.last_main_scene = 'inside'

        # Requested scene change from a behavior (e.g. go_to on arrival). Cleared by scene_manager.
        self.pending_scene = None

        # ESP-NOW manager; injected by Game.__init__ when WIFI_ENABLED. None otherwise.
        self.espnow = None

        # Pet display name. Derived from MAC on first social scene entry; None until then.
        self.pet_name = None

        # Active visit state. None when not visiting, otherwise:
        #   {'peer_mac': bytes, 'peer_name': str, 'role': str,
        #    'greeted': bool, 'play_time': float}
        self.visit = None

        # Friends: mac_hex_str -> {'n': name, 't': total_seconds, 'c': visit_count}
        self.friends = {}

        if delete_save:
            try:
                import uos
                uos.remove(_SAVE_PATH)
                print("[Context] Save file deleted")
            except:
                pass
        print("[Context] Reset to defaults")

    def record_behavior(self, name):
        """Prepend a completed behavior name; keeps the 5 most recent."""
        self.recent_behaviors.insert(0, name)
        if len(self.recent_behaviors) > 5:
            self.recent_behaviors.pop()

    def get_friendship_level(self, mac_hex):
        """Return 0.0-1.0 familiarity with a peer (by MAC hex string).

        Reaches 1.0 after 15 minutes of total playtime across all visits.
        """
        entry = self.friends.get(mac_hex)
        if not entry:
            return 0.0
        return min(1.0, entry.get('t', 0.0) / 900.0)

    def update_friend(self, mac_hex, name, seconds_added):
        """Record playtime with a peer. Creates a new entry or updates existing.

        Caps the friends list at 10 entries, evicting the one with least total time.
        """
        if mac_hex in self.friends:
            f = self.friends[mac_hex]
            f['n'] = name
            f['t'] = f.get('t', 0.0) + seconds_added
            f['c'] = f.get('c', 0) + 1
        else:
            if len(self.friends) >= 10:
                oldest = min(self.friends, key=lambda k: self.friends[k].get('t', 0.0))
                del self.friends[oldest]
            self.friends[mac_hex] = {'n': name, 't': seconds_added, 'c': 1}

    def record_visit_end(self):
        """Persist visit stats to the friends dict and clear active visit."""
        if self.visit:
            mac = self.visit.get('peer_mac')
            name = self.visit.get('peer_name', '?')
            secs = self.visit.get('play_time', 0.0)
            if mac and secs > 5:
                mac_hex = ':'.join('%02x' % b for b in mac)
                self.update_friend(mac_hex, name, secs)
                print('[Context] Recorded %.0fs with %s (%s)' % (secs, name, mac_hex))
        self.visit = None

    def save_if_needed(self):
        """Save+reboot if more than 59 minutes have passed since the last save."""
        import time
        if (self.last_save_time is None or
                time.ticks_diff(time.ticks_ms(), self.last_save_time) > 59 * 60 * 1000):
            self.save()