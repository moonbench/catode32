class GameContext:
    def __init__(self):
        # Rapidly changing stats (change on a daily basis)
        self.fullness = 50          # Inverse of hunger. Feed to maintain.
        self.stimulation = 50       # Engagement
        self.energy = 50            # How rested the pet is
        self.vigor = 50             # Inverse of exhaustion
        self.comfort = 50           # Physical comfort. Temperature, environment, etc...
        self.playfulness = 50       # Mood to play
        self.focus = 50             # Ability to concentrate on tasks/training

        # Slower changing stats (change on more of a weekly basis)
        self.health = 50            # Overall physical health
        self.fulfillment = 50       # Feeling like the pet has purpose and things to do
        self.cleanliness = 50       # How clean the pet and its environment are
        self.curiosity = 50         # Drive to explore/investigate
        self.confidence = 50        # Willingness to try new things
        self.independence = 50      # How happy it is to be solo (versus needing attention more)
        self.sociability = 50       # How interested the pet is in interacting
        self.routine = 50           # How comfortable the pet is with established patterns
        self.intelligence = 50      # Problem-solving, learning new skills/tricks
        self.resilience = 50        # Ability to bounce back from stress
        self.maturity = 50          # Behavioral sophistication
        self.grace = 50             # Physical elegance, landing movements well, etc
        self.boldness = 50          # Willingness to take risks, explore new places
        self.affection = 50         # How much the pet feels loved

        # Even slower changing stats (change on more of a monthly basis)
        self.fitness = 50           # Athleticism
        self.alertness = 50         # Mental sharpness
        self.appetite = 50          # Interest in food variety (different than fullness)
        self.wisdom = 50            # Life experience, decision making
        self.patience = 50          # Tolerance for waiting, being groomed
        self.charisma = 50          # Attractiveness to other pets
        self.craftiness = 50        # Cleverness in getting what they want
        self.serenity = 50          # Inner peace. Makes them less likely to be stressed

        # Slowest changing stats (basically traits with little or no change)
        self.courage = 50           # Reaction to new/scary situations
        self.loyalty = 50           # Attachment strength
        self.mischievousness = 50   # Tendency towards trouble
        self.sensitivity = 50       # Emotional responsiveness to environment
        self.dignity = 50           # How they carry themselves, respond to embarrassment

        # Inventory for menu testing
        self.inventory = {
            "toys": ["Feather", "Yarn ball", "Laser"],
            "snacks": ["Treat", "Kibble"],
        }

        # Minigame high scores
        self.zoomies_high_score = 0
        self.maze_best_time = 0  # Best time in seconds (0 = not played)

        # For storing time/weather/season/moon-phase type data
        self.environment = {}

        # Behavior override - set to behavior name to force that behavior next
        self.override_next_behavior = None