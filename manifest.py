# Frozen module manifest for petpython firmware build.
#
# Files frozen here have their bytes literals stored in flash rather than heap.
# Import paths are relative to "src/" so assets/character.py -> assets.character
#
# Only freeze always-loaded asset modules. Lazy-loaded scenes and behaviors
# must stay on the filesystem so they can be unloaded from sys.modules.

# Include the board's default manifest (asyncio, networking libs, etc.)
include("$(PORT_DIR)/boards/manifest.py")

freeze("/Users/user/Documents/petpython/src", (
    "assets/__init__.py",
    "assets/boot_img.py",
    "assets/character.py",
    "assets/effects.py",
    "assets/furniture.py",
    "assets/icons.py",
    "assets/items.py",
    "assets/minigame_assets.py",
    "assets/minigame_character.py",
    "assets/nature.py",
    "assets/store.py",
))
