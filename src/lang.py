"""
lang.py  —  Localisation / translation system for catode32
=============================================================

HOW TO SELECT A LANGUAGE
-------------------------
Change the LANGUAGE constant below:

    LANGUAGE = "nl"    # Dutch
    LANGUAGE = "en"    # English (default)

HOW TO ADD A LANGUAGE
---------------------
1. Copy the "en" block in _TRANSLATIONS.
2. Change the outer key to your ISO 639-1 code ("fr", "de", "es", …).
3. Translate every value. Keys must stay identical.
4. Open a pull request — that is all.

RULES FOR TRANSLATORS
---------------------
- Max ~16 characters per label (128 px / 8 px font).
- Keep {…} placeholders exactly as-is.
- Omit keys you have not translated; English is used as fallback.
"""

# ── Change this to switch language ──────────────────────────────────────────
LANGUAGE = "nl"
# ────────────────────────────────────────────────────────────────────────────


_TRANSLATIONS = {

    # ── English (source / fallback) ─────────────────────────────────────────
    "en": {

        # --- Main menu ---
        "Affection":        "Affection",
        "Feed":             "Feed",
        "Play":             "Play",
        "Gardening":        "Gardening",
        "Locations":        "Locations",
        "Minigames":        "Minigames",
        "Store":            "Store",
        "Store...":         "Store...",
        "Forecast":         "Forecast",
        "Social":           "Social",
        "Pet info":         "Pet info",
        "Pet stats":        "Pet stats",
        "Debug":            "Debug",

        # --- Affection ---
        "Pets":             "Pets",
        "Scratch":          "Scratch",
        "Kiss":             "Kiss",
        "Psst psst":        "Psst psst",
        "Groom":            "Groom",
        "Train":            "Train",

        # --- Training ---
        "Intelligence":     "Intelligence",
        "Behavior":         "Behavior",
        "Fitness":          "Fitness",
        "Sociability":      "Sociability",

        # --- Feed ---
        "Meals":            "Meals",
        "Snacks":           "Snacks",

        # --- Locations ---
        "Living Room":      "Living Room",
        "Bedroom":          "Bedroom",
        "Kitchen":          "Kitchen",
        "Outside":          "Outside",
        "Treehouse":        "Treehouse",

        # --- Vacations ---
        "Vacations":        "Vacations",
        "Aquarium":         "Aquarium",
        "Beach":            "Beach",
        "Forest":           "Forest",
        "Park":             "Park",
        "Around Here":      "Around Here",

        # --- Gardening ---
        "Tend":             "Tend",
        "Place Pot":        "Place Pot",
        "Plant Seed":       "Plant Seed",
        "In Pot":           "In Pot",
        "In Ground":        "In Ground",
        "Water":            "Water",
        "Fertilize":        "Fertilize",
        "Move":             "Move",
        "Repot":            "Repot",
        "Inspect":          "Inspect",
        "Garden":           "Garden",

        # --- Store categories ---
        "Food":             "Food",
        "Toys":             "Toys",
        "Plants":           "Plants",
        "Seeds":            "Seeds",
        "Pots":             "Pots",
        "Fertilizer":       "Fertilizer",
        "Tools":            "Tools",
        "Trips":            "Trips",
        "Exit":             "Exit",

        # --- Store food items ---
        "Kibble":           "Kibble",
        "Cod":              "Cod",
        "Haddock":          "Haddock",
        "Trout":            "Trout",
        "Shrimp":           "Shrimp",
        "Herring":          "Herring",
        "Turkey":           "Turkey",
        "Tuna":             "Tuna",
        "Salmon":           "Salmon",
        "Chicken":          "Chicken",
        "Liver":            "Liver",
        "Beef":             "Beef",
        "Lamb":             "Lamb",

        # --- Store snacks ---
        "Carrots":          "Carrots",
        "Pumpkin":          "Pumpkin",
        "Treats":           "Treats",
        "Bytes":            "Bytes",
        "Eggs":             "Eggs",
        "Nuggets":          "Nuggets",
        "Milk":             "Milk",
        "Sticks":           "Sticks",
        "Puree":            "Puree",

        # --- Store toys ---
        "String":           "String",
        "Feather":          "Feather",
        "Mouse":            "Mouse",
        "Yarn":             "Yarn",
        "Laser":            "Laser",

        # --- Store pots ---
        "Small":            "Small",
        "Medium":           "Medium",
        "Large":            "Large",
        "Planter":          "Planter",

        # --- Store plants ---
        "Grass":            "Grass",
        "Freesia":          "Freesia",
        "Sun":              "Sun",
        "Rose":             "Rose",

        # --- Store trips ---
        "Aqua.":            "Aqua.",

        # --- Store confirm messages ---
        "Can't afford!":            "Can't afford!",
        "Already owned!":           "Already owned!",
        "Replace: {cost}c":         "Replace: {cost}c",
        "Fertilizer: {cost}c":      "Fertilizer: {cost}c",
        "A trip to the {dest}: {cost}c": "A trip to the {dest}: {cost}c",

        # --- Minigames ---
        "Zoomies":          "Zoomies",
        "Breakout":         "Breakout",
        "Snake":            "Snake",
        "Hunter":           "Hunter",
        "Memory":           "Memory",
        "Maze":             "Maze",
        "TicTacToe":        "TicTacToe",
        "Hanjie":           "Hanjie",
        "Lights Out":       "Lights Out",
        "Pipes":            "Pipes",

        # --- In-game UI ---
        "GAME OVER":        "GAME OVER",
        "YOU WIN!":         "YOU WIN!",
        "WIN!":             "WIN!",
        "A: START":         "A: START",
        "A: Start":         "A: Start",
        "A: RETRY":         "A: RETRY",
        "Retry":            "Retry",
        "New Board":        "New Board",
        "Grid Size":        "Grid Size",
        "Flawless!":        "Flawless!",
        "SNAKE":            "SNAKE",
        "You":              "You",
        "Pet":              "Pet",
        "Par:":             "Par:",
        "Loading...":       "Loading...",

        # --- Social scene ---
        "No cats nearby...":  "No cats nearby...",
        "A=invite  B=back":   "A=invite  B=back",
        "Inviting...":        "Inviting...",

        # --- Pet info labels ---
        "Name: {name}":       "Name: {name}",
        "Age: {days} days":   "Age: {days} days",
        "Gender: {g}":        "Gender: {g}",
        "- Favorites -":      "- Favorites -",
        "Meal: {v}":          "Meal: {v}",
        "Snack: {v}":         "Snack: {v}",
        "Toy: {v}":           "Toy: {v}",
        "Place: {v}":         "Place: {v}",
        # Gender values
        "Male":               "Male",
        "Female":             "Female",
        # Toy display names
        "Yarn Ball":          "Yarn Ball",
        "Mouse Toy":          "Mouse Toy",
        "Laser Pointer":      "Laser Pointer",

        # --- Stats scene — stat names ---
        "Health":             "Health",
        "Fullness":           "Fullness",
        "Energy":             "Energy",
        "Comfort":            "Comfort",
        "Cleanliness":        "Cleanliness",
        "Focus":              "Focus",
        "Curiosity":          "Curiosity",
        "Playfulness":        "Playfulness",
        "Fulfillment":        "Fulfillment",
        "Serenity":           "Serenity",
        "Courage":            "Courage",
        "Loyalty":            "Loyalty",
        "Mischievousness":    "Mischievousness",
        "Maturity":           "Maturity",

        # --- Stats scene — stat descriptions ---
        "desc_health":
            "Overall wellbeing, derived from all other stats. No single action raises it directly. Keep fitness, fullness, energy, comfort, and affection balanced.",
        "desc_fullness":
            "How satisfied your pet's belly is. Feed treats and meals to fill it. Activity burns through it, especially sleep and exercise.",
        "desc_energy":
            "How rested and ready your pet is. Restored by sleeping. Burns down during playing, zoomies, training, and hunting.",
        "desc_comfort":
            "Physical ease with surroundings. Restored by sleep, stretching, and affection. Worn down by startles and prolonged inactivity.",
        "desc_cleanliness":
            "How clean your pet is. Your pet self-grooms regularly, and grooming gives a bigger boost. Activity gradually dirties them.",
        "desc_fitness":
            "Athletic conditioning. Built through training, hunting, zoomies, and movement. Fades slowly during rest.",
        "desc_focus":
            "Ability to concentrate. Restored by sleep and affection. Scattered by mischief and active exploration.",
        "desc_intelligence":
            "Problem solving ability. Developed through training and hunting. Fades slowly when left to idle.",
        "desc_curiosity":
            "Drive to explore. Sparked by rest, surprises, and your attention. Satisfied by investigating and observing.",
        "desc_playfulness":
            "Desire to play. Restored by sleep and affection. Spent through play, training, and zoomies.",
        "desc_affection":
            "How loved your pet feels. Filled by kisses, petting, grooming, and treats. Drains when your pet sulks or hides.",
        "desc_fulfillment":
            "Sense of purpose. Grows through training, grooming, and affection. Fades when stuck idling too long.",
        "desc_serenity":
            "Inner peace and calm. Restored by rest, kneading, and grooming. Worn by vocalizing and active exploration.",
        "desc_sociability":
            "Eagerness to interact. Boosted by training, affection, and grooming. Falls when hiding, sulking, or causing mischief.",
        "desc_courage":
            "Boldness in scary situations. Strengthened through training and affection. Chipped by startles and sulking.",
        "desc_loyalty":
            "Strength of bond. Grows through training and frequent affection. Eroded by mischief and sulking.",
        "desc_mischievousness":
            "Tendency toward playful trouble. Rises during mischief and hunting. Reduced by affection and training.",
        "desc_maturity":
            "Behavioral self-control. Develops through training and observing. Set back by mischief and zoomies.",


        # --- Memory minigame ---
        "Incredible!":     "Incredible!",
        "Amazing!":        "Amazing!",
        "Impressive!":     "Impressive!",
        "Well done!":      "Well done!",
        "Not bad!":        "Not bad!",
        "Phwew!":          "Phwew!",
        "Best: {n}":       "Best: {n}",
        "\n\nScore: {n}": "\n\nScore: {n}",

        # --- Maze ---
        "Found it!":       "Found it!",

        # --- Hanjie ---
        "Time: {v}":       "Time: {v}",
        "Best: {v}":       "Best: {v}",

        # --- Zoomies ---
        "+100":            "+100",
        "Ooof!":           "Ooof!",

        # --- Debug stats ---
        "Mischief":        "Mischief",

        # --- Forecast time ---
        "12A":   "12a",
        "12P":   "12p",
        "{h}A":  "{h}a",
        "{h}P":  "{h}p",
        # --- Forecast scene ---

        # --- Weather types ---
        "Clear":     "Clear",
        "Cloudy":    "Cloudy",
        "Overcast":  "Overcast",
        "Rain":      "Rain",
        "Storm":     "Storm",
        "Snow":      "Snow",
        "Windy":     "Windy",



        # --- Store confirm format strings ---
        "{item}({uses}): {cost}c":  "{item}({uses}): {cost}c",
        "{item}: {cost}c":          "{item}: {cost}c",
        "{item}x{n}: {cost}c":      "{item}x{n}: {cost}c",

        # --- Store full names (used in confirms) ---
        "Small pot":        "Small pot",
        "Medium pot":       "Medium pot",
        "Large pot":        "Large pot",
        "Planter box":      "Planter box",
        "Cat Grass":        "Cat Grass",
        "Freesia":          "Freesia",
        "Sunflower":        "Sunflower",
        "Rose":             "Rose",
        "Spade":            "Spade",
        "Watering Can":     "Watering Can",
        "String":           "String",
        "Feather":          "Feather",
        "Mouse Toy":        "Mouse Toy",
        "Yarn Ball":        "Yarn Ball",
        "Laser Pointer":    "Laser Pointer",
        "Groom":            "Groom",
        "Train":            "Train",

        # --- Debug missing ---
        "Plants":           "Plants",
        "Vacations":        "Vacations",
        "Environment":      "Environment",
        "Poses":            "Poses",

        # --- Contextual menu (menu2 / main_scene) ---
        "Pluck":                            "Pluck",
        "Remove plant?":                    "Remove plant?",
        "To {loc}":                         "To {loc}",
        "Too damaged! Buy a new one at the store.": "Too damaged! Buy a new one at the store.",
        "No empty pots in this location":   "No empty pots in this location",
        "Chew Stick":                       "Chew Stick",
        "Nugget":                           "Nugget",
        "Cream":                            "Cream",
        "Fish Bite":                        "Fish Bite",
        "Mackerel":                         "Mackerel",
        "Cat Grass":                        "Cat Grass",
        "Sunflower":                        "Sunflower",
        "Small pot":                        "Small pot",
        "Medium pot":                       "Medium pot",
        "Large pot":                        "Large pot",
        "Planter box":                      "Planter box",
        # --- Store purchase confirmations ---
        "{item} purchased!":        "{item} purchased!",
        "{item} replaced!":         "{item} replaced!",
        "{item} bought!":           "{item} bought!",
        "Fertilizer bought!":       "Fertilizer bought!",
        "A luxurious spa day!":     "A luxurious spa day!",
        "Professional training done!": "Professional training done!",
        "c":                        "c",

        # --- Store trips ---
        "Park":                     "Park",
        "Forest":                   "Forest",
        "Beach":                    "Beach",
        "Aqua.":                    "Aqua.",

        # --- Store tools & services ---
        "Spade":                    "Spade",
        "W. Can":                   "W. Can",
        "Groom":                    "Groom",
        "Train":                    "Train",

        # --- Debug behaviors ---
        "Idle":                     "Idle",
        "Sleeping":                 "Sleeping",
        "Napping":                  "Napping",
        "Stretching":               "Stretching",
        "Kneading":                 "Kneading",
        "Lounging":                 "Lounging",
        "Investigating":            "Investigating",
        "Startled":                 "Startled",
        "Observing":                "Observing",
        "Chattering":               "Chattering",
        "Vocalizing":               "Vocalizing",
        "Self Grooming":            "Self Grooming",
        "Being Groomed":            "Being Groomed",
        "Hunting":                  "Hunting",
        "Gift Bringing":            "Gift Bringing",
        "Pacing":                   "Pacing",
        "Meandering":               "Meandering",
        "Sulking":                  "Sulking",
        "Hiding":                   "Hiding",
        "Training":                 "Training",
        "Playing":                  "Playing",
        "Playing (ball)":           "Playing (ball)",
        "Attention":                "Attention",
        "Hearing (exclaim)":        "Hearing (exclaim)",
        "Hearing (heart)":          "Hearing (heart)",
        "Hearing (note)":           "Hearing (note)",
        "Eating":                   "Eating",
        "Eating (treat)":           "Eating (treat)",

        # --- Debug context ---
        "Reset all plants?":        "Reset all plants?",
        "Mem. Usage":       "Mem. Usage",
        "Coins":                    "Coins",
        "Seed":                     "Seed",

        # --- Credits ---
        "CREDITS": """\
   Catode 32   
===============

Thank you for
playing with
this virtual
pet!
_______________

Code & Design:
Moonbench
_______________

This pet was
inspired by two
wonderful cats.
""",
        # --- Settings / debug ---
        "Save now":                     "Save now",
        "Save and reboot?":             "Save and reboot?",
        "Reset stats":                  "Reset stats",
        "Reset Plants":                 "Reset Plants",
        "Reset all stats to defaults?": "Reset all stats to defaults?",
        "Time Speed":                   "Time Speed",
        "Behaviors":                    "Behaviors",
        "Stats":                        "Stats",
        "Power":                        "Power",
        "Power Control":                "Power Control",
        "Wifi":                         "Wifi",
        "Wireless":                     "Wireless",
        "ESP-NOW":                      "ESP-NOW",
        "RGB LED":                      "RGB LED",
        "Plant type":                   "Plant type",
        "Pot type":                     "Pot type",
        "Service":                      "Service",
        "Credits":                      "Credits",
        "Prowl":                        "Prowl",
        "Poses":                        "Poses",
        "Context":                      "Context",
        "Environment":                  "Environment",
    },

    # ── Dutch ────────────────────────────────────────────────────────────────
    "nl": {

        # --- Hoofdmenu ---
        "Affection":        "Genegenheid",
        "Feed":             "Voeren",
        "Play":             "Spelen",
        "Gardening":        "Tuinieren",
        "Locations":        "Locaties",
        "Minigames":        "Minigames",
        "Store":            "Winkel",
        "Store...":         "Winkel...",
        "Forecast":         "Weersvoorsp.",
        "Social":           "Sociaal",
        "Pet info":         "Huisdier info",
        "Pet stats":        "Statistieken",
        "Debug":            "Debug",

        # --- Genegenheid ---
        "Pets":             "Aaien",
        "Scratch":          "Krabben",
        "Kiss":             "Knuffelen",
        "Psst psst":        "Sst sst",
        "Groom":            "Verzorgen",
        "Train":            "Trainen",

        # --- Training ---
        "Intelligence":     "Intelligentie",
        "Behavior":         "Gedrag",
        "Fitness":          "Conditie",
        "Sociability":      "Sociabiliteit",

        # --- Voeren ---
        "Meals":            "Maaltijden",
        "Snacks":           "Snacks",

        # --- Locaties ---
        "Living Room":      "Woonkamer",
        "Bedroom":          "Slaapkamer",
        "Kitchen":          "Keuken",
        "Outside":          "Buiten",
        "Treehouse":        "Boomhut",

        # --- Vakanties ---
        "Vacations":        "Vakanties",
        "Aquarium":         "Aquarium",
        "Beach":            "Strand",
        "Forest":           "Bos",
        "Park":             "Park",
        "Around Here":      "In de buurt",

        # --- Tuinieren ---
        "Tend":             "Verzorgen",
        "Place Pot":        "Pot plaatsen",
        "Plant Seed":       "Zaad planten",
        "In Pot":           "In pot",
        "In Ground":        "In de grond",
        "Water":            "Begieten",
        "Fertilize":        "Bemesten",
        "Move":             "Verplaatsen",
        "Repot":            "Verpotten",
        "Inspect":          "Bekijken",
        "Garden":           "Tuin",

        # --- Winkel categorieën ---
        "Food":             "Eten",
        "Toys":             "Speelgoed",
        "Plants":           "Planten",
        "Seeds":            "Zaden",
        "Pots":             "Potten",
        "Fertilizer":       "Meststof",
        "Tools":            "Gereedschap",
        "Trips":            "Uitstapjes",
        "Exit":             "Sluiten",

        # --- Winkel eten ---
        "Kibble":           "Brokjes",
        "Cod":              "Kabeljauw",
        "Haddock":          "Schelvis",
        "Trout":            "Forel",
        "Shrimp":           "Garnalen",
        "Herring":          "Haring",
        "Turkey":           "Kalkoen",
        "Tuna":             "Tonijn",
        "Salmon":           "Zalm",
        "Chicken":          "Kip",
        "Liver":            "Lever",
        "Beef":             "Rund",
        "Lamb":             "Lam",

        # --- Winkel snacks ---
        "Carrots":          "Wortels",
        "Pumpkin":          "Pompoen",
        "Treats":           "Snoepjes",
        "Bytes":            "Hapjes",
        "Eggs":             "Eieren",
        "Nuggets":          "Nuggets",
        "Milk":             "Melk",
        "Sticks":           "Sticks",
        "Puree":            "Puree",

        # --- Winkel speelgoed ---
        "String":           "Touwtje",
        "Feather":          "Veer",
        "Mouse":            "Muis",
        "Yarn":             "Bol wol",
        "Laser":            "Laser",

        # --- Winkel potten ---
        "Small":            "Klein",
        "Medium":           "Middel",
        "Large":            "Groot",
        "Planter":          "Plantenbak",

        # --- Winkel planten ---
        "Grass":            "Gras",
        "Freesia":          "Freesia",
        "Sun":              "Zonnebloem",
        "Rose":             "Roos",

        # --- Winkel trips ---
        "Aqua.":            "Aquarium",


        # --- Winkel bevestigingsformaten ---
        "{item}({uses}): {cost}c":  "{item}({uses}): {cost}m",
        "{item}: {cost}c":          "{item}: {cost}m",
        "{item}x{n}: {cost}c":      "{item}x{n}: {cost}m",

        # --- Volledige namen voor bevestigingen ---
        "Small pot":        "Kleine pot",
        "Medium pot":       "Middellange pot",
        "Large pot":        "Grote pot",
        "Planter box":      "Plantenbak",
        "Cat Grass":        "Kattengras",
        "Sunflower":        "Zonnebloem",
        "Spade":            "Schep",
        "Watering Can":     "Gieter",
        "String":           "Touwtje",
        "Feather":          "Veer",
        "Mouse Toy":        "Muisspeeltje",
        "Yarn Ball":        "Bol wol",
        "Laser Pointer":    "Laserpen",
        "Groom":            "Verzorgen",
        "Train":            "Trainen",

        # --- Debug ontbrekend ---
        "Plants":           "Planten",
        "Vacations":        "Vakanties",
        "Environment":      "Omgeving",
        "Poses":            "Houdingen",

        # --- Contextueel menu (menu2 / main_scene) ---
        "Pluck":                            "Verwijderen",
        "Remove plant?":                    "Plant verwijderen?",
        "To {loc}":                         "Naar {loc}",
        "Too damaged! Buy a new one at the store.": "Te beschadigd! Koop een nieuw in de winkel.",
        "No empty pots in this location":   "Geen lege potten hier",
        "Chew Stick":                       "Kauwstick",
        "Nugget":                           "Nugget",
        "Cream":                            "Room",
        "Fish Bite":                        "Vissnack",
        "Mackerel":                         "Makreel",
        "Cat Grass":                        "Kattengras",
        "Sunflower":                        "Zonnebloem",
        "Small pot":                        "Kleine pot",
        "Medium pot":                       "Middelgrote pot",
        "Large pot":                        "Grote pot",
        "Planter box":                      "Plantenbak",
        # --- Winkel bevestigingen ---
        "Can't afford!":             "Niet genoeg!",
        "Already owned!":            "Al in bezit!",
        "Replace: {cost}c":          "Vervangen: {cost}m",
        "Fertilizer: {cost}c":       "Meststof: {cost}m",
        "A trip to the {dest}: {cost}c": "Uitstap naar {dest}: {cost}m",

        # --- Minigames ---
        "Zoomies":          "Zoomies",
        "Breakout":         "Breakout",
        "Snake":            "Slang",
        "Hunter":           "Jager",
        "Memory":           "Memory",
        "Maze":             "Doolhof",
        "TicTacToe":        "Boter-Kaas",
        "Hanjie":           "Hanjie",
        "Lights Out":       "Lichten Uit",
        "Pipes":            "Pijpen",

        # --- In-game UI ---
        "GAME OVER":        "GAME OVER",
        "YOU WIN!":         "GEWONNEN!",
        "WIN!":             "GEWONNEN!",
        "A: START":         "A: START",
        "A: Start":         "A: Start",
        "A: RETRY":         "A: OPNIEUW",
        "Retry":            "Opnieuw",
        "New Board":        "Nieuw bord",
        "Grid Size":        "Rastergrootte",
        "Flawless!":        "Perfect!",
        "SNAKE":            "SLANG",
        "You":              "Jij",
        "Pet":              "Huisdier",
        "Par:":             "Par:",
        "Loading...":       "Laden...",

        # --- Sociaal ---
        "No cats nearby...":  "Geen katten...",
        "A=invite  B=back":   "A=uitnodigen B=terug",
        "Inviting...":        "Uitnodigend...",

        # --- Huisdier info ---
        "Name: {name}":      "Naam: {name}",
        "Age: {days} days":  "Leeftijd: {days}d",
        "Gender: {g}":       "Geslacht: {g}",
        "- Favorites -":     "- Favorieten -",
        "Meal: {v}":         "Maaltijd: {v}",
        "Snack: {v}":        "Snack: {v}",
        "Toy: {v}":          "Speelgoed: {v}",
        "Place: {v}":        "Plek: {v}",
        "Male":              "Mannelijk",
        "Female":            "Vrouwelijk",
        "Yarn Ball":         "Bol wol",
        "Mouse Toy":         "Muisspeeltje",
        "Laser Pointer":     "Laserpen",

        # --- Statistieken namen ---
        "Health":            "Gezondheid",
        "Fullness":          "Verzadiging",
        "Energy":            "Energie",
        "Comfort":           "Comfort",
        "Cleanliness":       "Netheid",
        "Focus":             "Focus",
        "Curiosity":         "Nieuwsgierigheid",
        "Playfulness":       "Speelsheid",
        "Fulfillment":       "Vervulling",
        "Serenity":          "Sereniteit",
        "Courage":           "Moed",
        "Loyalty":           "Loyaliteit",
        "Mischievousness":   "Ondeugenheid",
        "Maturity":          "Rijpheid",

        # --- Statistieken omschrijvingen ---
        "desc_health":
            "Algemeen welzijn, afgeleid van alle andere statistieken. Houd fitness, verzadiging, energie, comfort en genegenheid in balans.",
        "desc_fullness":
            "Hoe tevreden de buik van je huisdier is. Voer traktaties en maaltijden. Activiteit verbruikt het snel.",
        "desc_energy":
            "Hoe uitgerust je huisdier is. Herstelt door slapen. Slijt tijdens spelen, zoomies, trainen en jagen.",
        "desc_comfort":
            "Fysiek gemak in de omgeving. Herstelt door slapen, strekken en genegenheid. Slijt door schrik en langdurige inactiviteit.",
        "desc_cleanliness":
            "Hoe schoon je huisdier is. Ze verzorgen zichzelf regelmatig; jij groomen geeft een grotere boost.",
        "desc_fitness":
            "Atletische conditie. Opgebouwd door trainen, jagen, zoomies en beweging. Neemt langzaam af tijdens rust.",
        "desc_focus":
            "Concentratievermogen. Herstelt door slapen en genegenheid. Verstrooid door ondeugendheid en verkenning.",
        "desc_intelligence":
            "Probleemoplossend vermogen. Ontwikkeld door trainen en jagen. Neemt langzaam af bij inactiviteit.",
        "desc_curiosity":
            "Drang om te verkennen. Aangewakkerd door rust, verrassingen en jouw aandacht. Bevredigd door onderzoeken.",
        "desc_playfulness":
            "Speeldrang. Herstelt door slapen en genegenheid. Gebruikt tijdens spelen en zoomies.",
        "desc_affection":
            "Hoe geliefd je huisdier zich voelt. Gevuld door knuffelen, aaien en verzorgen. Loopt leeg bij pruilen.",
        "desc_fulfillment":
            "Gevoel van doelgerichtheid. Groeit door trainen, verzorgen en genegenheid. Neemt af bij te lang niets doen.",
        "desc_serenity":
            "Innerlijke rust. Herstelt door rusten en kneden. Slijt door vocaliseren en actieve verkenning.",
        "desc_sociability":
            "Bereidheid tot interactie. Vergroot door trainen en genegenheid. Neemt af bij verstoppertje spelen en ondeugendheid.",
        "desc_courage":
            "Durf in enge situaties. Versterkt door trainen en genegenheid. Verminderd door schrik en pruilen.",
        "desc_loyalty":
            "Kracht van de band. Groeit door trainen en genegenheid. Aangetast door ondeugendheid en pruilen.",
        "desc_mischievousness":
            "Neiging tot speelse streken. Stijgt tijdens ondeugendheid. Vermindert door genegenheid en trainen.",
        "desc_maturity":
            "Gedragsbeheersing. Ontwikkelt door trainen en observeren. Teruggedraaid door ondeugendheid en zoomies.",


        # --- Memory minigame ---
        "Incredible!":     "Ongelooflijk!",
        "Amazing!":        "Geweldig!",
        "Impressive!":     "Indrukwekkend!",
        "Well done!":      "Goed gedaan!",
        "Not bad!":        "Niet slecht!",
        "Phwew!":          "Oef!",
        "Best: {n}":       "Best: {n}",
        "\n\nScore: {n}": "\n\nScore: {n}",

        # --- Doolhof ---
        "Found it!":       "Gevonden!",

        # --- Hanjie ---
        "Time: {v}":       "Tijd: {v}",
        "Best: {v}":       "Best: {v}",

        # --- Zoomies ---
        "+100":            "+100",
        "Ooof!":           "Oef!",

        # --- Debug statistieken ---
        "Mischief":        "Ondeugd",

        # --- Weersvoorspelling tijd ---
        "12A":   "12u",
        "12P":   "12m",
        "{h}A":  "{h}u",
        "{h}P":  "{h}m",
        # --- Weersvoorspelling ---
        "Clear":     "Helder",
        "Cloudy":    "Bewolkt",
        "Overcast":  "Betrokken",
        "Rain":      "Regen",
        "Storm":     "Storm",
        "Snow":      "Sneeuw",
        "Windy":     "Winderig",



        # --- Winkel bevestigingsformaten ---
        "{item}({uses}): {cost}c":  "{item}({uses}): {cost}m",
        "{item}: {cost}c":          "{item}: {cost}m",
        "{item}x{n}: {cost}c":      "{item}x{n}: {cost}m",

        # --- Volledige namen voor bevestigingen ---
        "Small pot":        "Kleine pot",
        "Medium pot":       "Middellange pot",
        "Large pot":        "Grote pot",
        "Planter box":      "Plantenbak",
        "Cat Grass":        "Kattengras",
        "Sunflower":        "Zonnebloem",
        "Spade":            "Schep",
        "Watering Can":     "Gieter",
        "String":           "Touwtje",
        "Feather":          "Veer",
        "Mouse Toy":        "Muisspeeltje",
        "Yarn Ball":        "Bol wol",
        "Laser Pointer":    "Laserpen",
        "Groom":            "Verzorgen",
        "Train":            "Trainen",

        # --- Debug ontbrekend ---
        "Plants":           "Planten",
        "Vacations":        "Vakanties",
        "Environment":      "Omgeving",
        "Poses":            "Houdingen",

        # --- Contextueel menu (menu2 / main_scene) ---
        "Pluck":                            "Verwijderen",
        "Remove plant?":                    "Plant verwijderen?",
        "To {loc}":                         "Naar {loc}",
        "Too damaged! Buy a new one at the store.": "Te beschadigd! Koop een nieuw in de winkel.",
        "No empty pots in this location":   "Geen lege potten hier",
        "Chew Stick":                       "Kauwstick",
        "Nugget":                           "Nugget",
        "Cream":                            "Room",
        "Fish Bite":                        "Vissnack",
        "Mackerel":                         "Makreel",
        "Cat Grass":                        "Kattengras",
        "Sunflower":                        "Zonnebloem",
        "Small pot":                        "Kleine pot",
        "Medium pot":                       "Middelgrote pot",
        "Large pot":                        "Grote pot",
        "Planter box":                      "Plantenbak",
        # --- Winkel bevestigingen ---
        "{item} purchased!":        "{item} gekocht!",
        "{item} replaced!":         "{item} vervangen!",
        "{item} bought!":           "{item} aangeschaft!",
        "Fertilizer bought!":       "Meststof aangeschaft!",
        "A luxurious spa day!":     "Een luxe spa dag!",
        "Professional training done!": "Professionele training klaar!",
        "c":                        "m",

        # --- Winkel trips ---
        "Park":                     "Park",
        "Forest":                   "Bos",
        "Beach":                    "Strand",
        "Aqua.":                    "Aquarium",

        # --- Winkel tools & diensten ---
        "Spade":                    "Schep",
        "W. Can":                   "Gieter",
        "Groom":                    "Verzorgen",
        "Train":                    "Trainen",

        # --- Debug gedragingen ---
        "Idle":                     "Niets doen",
        "Sleeping":                 "Slapen",
        "Napping":                  "Dutje doen",
        "Stretching":               "Uitrekken",
        "Kneading":                 "Kneden",
        "Lounging":                 "Lummelen",
        "Investigating":            "Onderzoeken",
        "Startled":                 "Schrikken",
        "Observing":                "Observeren",
        "Chattering":               "Klappertanden",
        "Vocalizing":               "Geluid maken",
        "Self Grooming":            "Zelf verzorgen",
        "Being Groomed":            "Verzorgd worden",
        "Hunting":                  "Jagen",
        "Gift Bringing":            "Cadeautje brengen",
        "Pacing":                   "IJsberen",
        "Meandering":               "Ronddwalen",
        "Sulking":                  "Pruilen",
        "Hiding":                   "Verstoppen",
        "Training":                 "Trainen",
        "Playing":                  "Spelen",
        "Playing (ball)":           "Spelen (bal)",
        "Attention":                "Aandacht",
        "Hearing (exclaim)":        "Horen (uitroep)",
        "Hearing (heart)":          "Horen (hart)",
        "Hearing (note)":           "Horen (noot)",
        "Eating":                   "Eten",
        "Eating (treat)":           "Snack eten",

        # --- Debug context ---
        "Reset all plants?":        "Alle planten resetten?",
        "Mem. Usage":       "Geheugengebr.",
        "Coins":                    "Munten",
        "Seed":                     "Zaad",

        # --- Credits ---
        "CREDITS": """\
   Catode 32   
===============

Bedankt voor
het spelen met
dit virtuele
huisdier!
_______________

Code & Design:
Moonbench
_______________

Vertaling NL:
Jouw naam hier
_______________

Dit huisdier
was geinspireerd
door twee
prachtige katten.
""",
        # --- Instellingen ---
        "Save now":                     "Nu opslaan",
        "Save and reboot?":             "Opslaan & herstarten?",
        "Reset stats":                  "Stats resetten",
        "Reset Plants":                 "Planten resetten",
        "Reset all stats to defaults?": "Alles resetten?",
        "Time Speed":                   "Tijdsnelheid",
        "Behaviors":                    "Gedragingen",
        "Stats":                        "Statistieken",
        "Power":                        "Stroom",
        "Power Control":                "Stroombeheer",
        "Credits":                      "Credits",
        "Prowl":                        "Sluipen",
        "Service":                      "Dienst",
    },

    # ── German (Deutsch) — community contribution welcome ───────────────────
    # "de": { ... },
}


# ── Translation lookup ───────────────────────────────────────────────────────

def t(key, **kwargs):
    """
    Translate *key* into the active LANGUAGE.
    Falls back to English, then to the key itself.
    Keyword arguments fill {placeholder} values in the result.
    """
    lang_dict = _TRANSLATIONS.get(LANGUAGE, {})
    en_dict   = _TRANSLATIONS.get("en", {})
    result    = lang_dict.get(key) or en_dict.get(key) or key
    if kwargs:
        try:
            result = result.format_map(kwargs)
        except (KeyError, ValueError):
            pass
    return result
