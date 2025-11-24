class GameContext:
    def __init__(self):

        self.char = {
            "blink": 0.0,
            "tail": 0.0,
        }

        self.fullness = 50 # Inverse of hunger
        self.stimulation = 50
        self.energy = 50
        self.vigor = 50 # Inverse of exhaustion
        self.comfort = 50
        self.playfulness = 50
        self.comfort = 50
        self.focus = 50
        self.affection = 50

        self.health = 50
        self.fulfillment = 50
        self.cleanliness = 50
        self.curiosity = 50
        self.confidence = 50