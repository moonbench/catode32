_SAVE_PATH = '/save.json'

_STAT_KEYS = (
    'fullness', 'energy', 'comfort', 'playfulness', 'focus',
    'fulfillment', 'cleanliness', 'curiosity', 'sociability',
    'intelligence', 'maturity', 'affection',
    'fitness', 'serenity',
    'courage', 'loyalty', 'mischievousness',
    'zoomies_high_score', 'maze_best_time', 'time_speed',
)


class GameContext:
    def __init__(self):
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
        self.curiosity = 50         # Drive to explore/investigate
        self.sociability = 50       # How interested the pet is in interacting
        self.intelligence = 50      # Problem-solving, learning new skills/tricks
        self.maturity = 50          # Behavioral sophistication
        self.affection = 50         # How much the pet feels loved

        # Even slower changing stats (change on more of a monthly basis)
        self.fitness = 50           # Athleticism
        self.serenity = 50          # Inner peace. Makes them less likely to be stressed

        # Slowest changing stats (basically traits with little or no change)
        self.courage = 50           # Reaction to new/scary situations
        self.loyalty = 50           # Attachment strength
        self.mischievousness = 50   # Tendency towards trouble

        # Inventory for menu testing
        self.inventory = {
            "toys": [
                {"name": "Feather", "variant": "toy"},
                {"name": "Yarn ball", "variant": "ball"},
                {"name": "Laser", "variant": "laser"},
            ],
            "snacks": [
                {"name": "Treat"},
                {"name": "Kibble"},
            ],
        }

        # Minigame high scores
        self.zoomies_high_score = 0
        self.maze_best_time = 0  # Best time in seconds (0 = not played)

        # For storing time/weather/season/moon-phase type data
        self.environment = {}

        # Debug: time scale multiplier (1.0 = normal, 2.0 = 2x speed, 0.0 = paused)
        self.time_speed = 1.0

        # Scene bounds for character movement (world coordinates, set by each scene on load)
        self.scene_x_min = 10
        self.scene_x_max = 118

        # Time of last save in ticks_ms; None = never saved this session
        self.last_save_time = None

        # Recent completed behavior names for loop prevention (most recent first, not persisted)
        self.recent_behaviors = []

        # Name of the most recently started behavior (not persisted, used to restore on scene re-entry)
        self.current_behavior_name = None

        # Last active "main" scene (inside/outside/etc) - used by secondary scenes to return home
        self.last_main_scene = 'inside'

        # Requested scene change from a behavior (e.g. go_to on arrival). Cleared by scene_manager.
        self.pending_scene = None
    
    def recompute_health(self):
        """Recompute health as a weighted average of contributing stats.

        Called after each behavior completes and applies its stat changes.
        Health is never modified directly — it is always derived.
        """
        raw = (
            0.20 * self.fitness +
            0.15 * self.fullness +
            0.15 * self.energy +
            0.10 * self.cleanliness +
            0.10 * self.comfort +
            0.10 * self.affection +
            0.05 * self.fulfillment +
            0.05 * self.focus +
            0.05 * self.intelligence +
            0.05 * self.playfulness
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

    def save(self):
        """Serialize stats to flash storage."""
        import ujson
        import time
        data = {'v': 1, 'env': self.environment}
        for key in _STAT_KEYS:
            data[key] = getattr(self, key)
        try:
            with open(_SAVE_PATH, 'w') as f:
                ujson.dump(data, f)
            import uos
            uos.sync()
            self.last_save_time = time.ticks_ms()
            import sys
            if '/remote' in sys.path:
                # Running under mpremote mount (dev mode) — soft reset would
                # kill the mount and crash mpremote, so skip it.
                print("[Context] Saved to " + _SAVE_PATH + " (dev mode, no reboot)")
            else:
                print("[Context] Saved to " + _SAVE_PATH + ", rebooting...")
                import machine
                machine.soft_reset()
        except Exception as e:
            print("[Context] Save failed: " + str(e))

    def load(self):
        """Load stats from flash storage. Returns True if successful."""
        import ujson
        try:
            with open(_SAVE_PATH, 'r') as f:
                data = ujson.load(f)
            for key in _STAT_KEYS:
                if key in data:
                    setattr(self, key, data[key])
            self.environment = data.get('env', {})
            self.recompute_health()
            import time
            self.last_save_time = time.ticks_ms()
            print("[Context] Loaded from " + _SAVE_PATH)
            return True
        except Exception as e:
            print("[Context] Load skipped: " + str(e))
            return False

    def reset(self):
        """Reset all stats to defaults and delete save file."""
        self.health = 50
        self.fullness = 50
        self.energy = 50
        self.comfort = 50
        self.playfulness = 50
        self.focus = 50
        self.fulfillment = 50
        self.cleanliness = 50
        self.curiosity = 50
        self.sociability = 50
        self.intelligence = 50
        self.maturity = 50
        self.affection = 50
        self.fitness = 50
        self.serenity = 50
        self.courage = 50
        self.loyalty = 50
        self.mischievousness = 50
        self.zoomies_high_score = 0
        self.maze_best_time = 0
        self.environment = {}
        self.time_speed = 1.0
        self.last_save_time = None
        self.recent_behaviors = []
        self.current_behavior_name = None
        self.last_main_scene = 'inside'
        self.pending_scene = None
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

    def save_if_needed(self):
        """Save if more than 59 minutes have passed since the last save."""
        import time
        if (self.last_save_time is None or
                time.ticks_diff(time.ticks_ms(), self.last_save_time) > 59 * 60 * 1000):
            self.save()