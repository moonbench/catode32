_SAVE_PATH = '/save.json'


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
        self.pending_intent = None  # scene name being loaded; set for crash handler

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
                'fertilizer': self.inventory.get("fertilizer", 0),
                'plants': self.plants,
                'next_plant_id': self.next_plant_id,
                'pet_seed': self.pet_seed,
                'pet_gender': self.pet_gender,
                'fav_meal': self.fav_meal, 'least_fav_meal': self.least_fav_meal,
                'fav_snack': self.fav_snack, 'least_fav_snack': self.least_fav_snack,
                'fav_toy': self.fav_toy, 'least_fav_toy': self.least_fav_toy,
                'fav_location': self.fav_location, 'least_fav_location': self.least_fav_location,
                'wifi_familiar': self.wifi_familiar, 'wifi_recent': self.wifi_recent,
                'pet_name': self.pet_name, 'friends': self.friends,
                'recent_meals': self.recent_meals}
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
        import reset_context
        reset_context.reset_plants(self)
        import sys
        sys.modules.pop('reset_context', None)

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
            self.pet_seed = data['pet_seed']
            _FAVOR_KEYS = ('pet_gender', 'fav_meal', 'least_fav_meal',
                           'fav_snack', 'least_fav_snack',
                           'fav_toy', 'least_fav_toy',
                           'fav_location', 'least_fav_location')
            if any(k not in data for k in _FAVOR_KEYS):
                import reset_context
                _favs = reset_context._derive_favorites(self.pet_seed)
                import sys
                sys.modules.pop('reset_context', None)
            else:
                _favs = {}
            for _k in _FAVOR_KEYS:
                setattr(self, _k, data.get(_k, _favs.get(_k)))
            self.pet_name = data.get('pet_name')
            self.environment = data.get('env', {})
            if 'food_stock' in data:
                self.food_stock.update(data['food_stock'])
            if 'toys' in data:
                _valid_variants = {'string', 'feather', 'ball', 'laser'}
                self.inventory['toys'] = [
                    t for t in data['toys']
                    if t.get('variant') in _valid_variants
                ]
            if 'pots' in data:
                self.inventory['pots'].update(data['pots'])
            if 'seeds' in data:
                self.inventory['seeds'].update(data['seeds'])
            if 'tools' in data:
                self.inventory['tools'].update(data['tools'])
            if 'fertilizer' in data:
                self.inventory['fertilizer'] = data['fertilizer']
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
            self.recent_meals  = data.get('recent_meals',  [])
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
        import reset_context
        reset_context.do_reset(self, delete_save)
        import sys
        sys.modules.pop('reset_context', None)

    def record_behavior(self, name):
        """Prepend a completed behavior name; keeps the 5 most recent."""
        self.recent_behaviors.insert(0, name)
        if len(self.recent_behaviors) > 5:
            self.recent_behaviors.pop()

    def record_meal(self, food_type):
        """Prepend a completed meal; keeps the 5 most recent."""
        self.recent_meals.insert(0, food_type)
        if len(self.recent_meals) > 5:
            self.recent_meals.pop()

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