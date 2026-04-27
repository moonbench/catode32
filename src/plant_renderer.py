"""plant_renderer.py - Drawing callbacks for the living plant system.

Call register_plant_draws(scene) from a MainScene subclass's enter() path
(via the base-class _register_plant_draws hook) to attach per-layer draw
callbacks onto the scene's Environment.
"""

import config
from assets.plants import PLANT_SPRITES, POT_SPRITES


def _rebuild_plant_cache(scene):
    """Build scene._plant_layer_cache: {layer: [plant, ...]} for this scene only."""
    cache = {}
    scene_name = scene.SCENE_NAME
    for plant in scene.context.plants:
        if plant['scene'] == scene_name:
            layer = plant['layer']
            if layer not in cache:
                cache[layer] = []
            cache[layer].append(plant)
    scene._plant_layer_cache = cache


def invalidate_plant_cache(scene):
    """Mark the plant layer cache stale so it is rebuilt on the next draw."""
    scene._plant_layer_cache = None


def register_plant_draws(scene):
    """Register a draw callback for each unique layer in scene.PLANT_SURFACES."""
    scene._plant_sprites = PLANT_SPRITES
    scene._pot_sprites   = POT_SPRITES
    _rebuild_plant_cache(scene)
    seen = set()
    for surf in getattr(scene, 'PLANT_SURFACES', []):
        layer = surf['layer']
        if layer in seen:
            continue
        seen.add(layer)
        def make_cb(l):
            def cb(renderer, camera_x, parallax):
                draw_plants_layer(scene, renderer, camera_x, parallax, l)
            return cb
        scene.environment.add_custom_draw(layer, make_cb(layer))



def draw_plants_layer(scene, renderer, camera_x, parallax, layer):
    """Draw all pots and plants for one layer of one scene."""
    cache = getattr(scene, '_plant_layer_cache', None)
    if cache is None:
        _rebuild_plant_cache(scene)
        cache = scene._plant_layer_cache

    plants = cache.get(layer)
    if not plants:
        return

    plant_sprites = scene._plant_sprites
    pot_sprites   = scene._pot_sprites
    offset        = int(camera_x * parallax)
    plant_bursts  = getattr(scene, '_plant_bursts', None)

    for plant in plants:
        screen_x = plant['x'] - offset
        if screen_x + 30 < 0 or screen_x > config.DISPLAY_WIDTH:
            continue

        y_snap   = plant.get('y_snap', 63)
        pot_type = plant['pot']
        stage    = plant['stage']
        mirror   = plant.get('mirror', False)

        pot_h = 0
        pot_w = 0
        if pot_type != 'ground':
            pot_spr = pot_sprites[pot_type]
            pot_h = pot_spr['height']
            pot_w = pot_spr['width']
            renderer.draw_sprite_obj(pot_spr, screen_x, y_snap - pot_h, mirror_h=mirror)

        if stage not in ('empty_pot', 'dead', 'dormant') and stage is not None:
            plant_spr = plant_sprites[(plant['type'], stage)]
            if plant_spr:
                cx = screen_x + pot_w // 2
                plant_x = cx - plant_spr['width'] // 2
                renderer.draw_sprite_obj(
                    plant_spr,
                    plant_x,
                    y_snap - pot_h - plant_spr['height'],
                    mirror_h=mirror,
                )
                if plant_bursts:
                    effect = plant_bursts.get(plant['id'])
                    if effect:
                        mid_y = y_snap - pot_h - plant_spr['height'] // 2
                        effect.draw(renderer, cx, mid_y)
