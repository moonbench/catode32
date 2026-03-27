"""splash.py - Boot splash screen."""

import config
from assets.boot_img import STRETCH_CAT1


def show_splash(renderer):
    """Draw the loading splash and push it to the display immediately."""
    renderer.clear()
    sprite_x = (config.DISPLAY_WIDTH - STRETCH_CAT1["width"]) // 2
    sprite_y = 10
    renderer.draw_sprite_obj(STRETCH_CAT1, sprite_x, sprite_y)
    # "Loading..." is 10 chars * 8px = 80px wide
    text_x = (config.DISPLAY_WIDTH - 80) // 2
    text_y = sprite_y + STRETCH_CAT1["height"] + 6
    renderer.draw_text("Loading...", text_x, text_y)
    renderer.show()
