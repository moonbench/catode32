class GameContext:
    def __init__(self):
        # Rapidly changing stats (change on a daily basis)
        self.fullness = 50          # Inverse of hunger. Feed to maintain.
        self.energy = 50            # How rested the pet is
        self.comfort = 50           # Physical comfort. Temperature, environment, etc...
        self.playfulness = 50       # Mood to play
        self.focus = 50             # Ability to concentrate on tasks/training

        # Slower changing stats (change on more of a weekly basis)
        self.health = 50            # Overall physical health
        self.fulfillment = 50       # Feeling like the pet has purpose and things to do
        self.cleanliness = 50       # How clean the pet and its environment are
        self.curiosity = 50         # Drive to explore/investigate
        self.sociability = 50       # How interested the pet is in interacting
        self.intelligence = 50      # Problem-solving, learning new skills/tricks
        self.maturity = 50          # Behavioral sophistication
        self.affection = 50         # How much the pet feels loved

        # Even slower changing stats (change on more of a monthly basis)
        self.fitness = 50           # Athleticism
        self.patience = 50          # Tolerance for waiting, being groomed
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
    
    def debug_print_stats(self):
        print("Stats:")
        print("Fullness:     %6.4f, Energy:       %6.4f, Comfort:         %6.4f" % (self.fullness, self.energy, self.comfort))
        print("Playfulness:  %6.4f, Focus:        %6.4f" % (self.playfulness, self.focus))
        print("----------------------------------------------------------------")
        print("Health:       %6.4f, Fulfillment:  %6.4f, Cleanliness:     %6.4f" % (self.health, self.fulfillment, self.cleanliness))
        print("Curiosity:    %6.4f, Sociability:     %6.4f" % (self.curiosity, self.sociability))
        print("Intelligence: %6.4f" % (self.intelligence))
        print("Maturity:     %6.4f, Affection:    %6.4f" % (self.maturity, self.affection))
        print("----------------------------------------------------------------")
        print("Fitness:      %6.4f, Patience:     %6.4f" % (self.fitness, self.patience))
        print("Serenity:     %6.4f" % (self.serenity))
        print("----------------------------------------------------------------")
        print("Courage:      %6.4f, Loyalty:      %6.4f, Mischievousness: %6.4f" % (self.courage, self.loyalty, self.mischievousness))
        print("----------------------------------------------------------------")