"""plant_renderer.py - Drawing callbacks for the living plant system.

Call register_plant_draws(scene) from a MainScene subclass's enter() path
(via the base-class _register_plant_draws hook) to attach per-layer draw
callbacks onto the scene's Environment.
"""

import config
from assets.plants import PLANT_SPRITES, POT_SPRITES
from assets.effects import BURST1

_BURST_FRAME_DUR = 1.0 / BURST1['speed']
_BURST_TOTAL = len(BURST1['frames']) * _BURST_FRAME_DUR


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


def _draw_plant_bursts(renderer, info, cx, mid_y):
    """Draw sparkle bursts centred at (cx, mid_y) for a recently-watered plant."""
    timer = info['timer']
    hw = BURST1['width'] // 2
    hh = BURST1['height'] // 2
    for burst in info['bursts']:
        elapsed = timer - burst['delay']
        if elapsed < 0 or elapsed >= _BURST_TOTAL:
            continue
        frame_idx = min(int(elapsed / _BURST_FRAME_DUR), len(BURST1['frames']) - 1)
        renderer.draw_sprite(
            BURST1['frames'][frame_idx],
            BURST1['width'], BURST1['height'],
            cx + burst['dx'] - hw, mid_y + burst['dy'] - hh,
            transparent=True, transparent_color=0,
        )


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
                    burst_info = plant_bursts.get(plant['id'])
                    if burst_info:
                        mid_y = y_snap - pot_h - plant_spr['height'] // 2
                        _draw_plant_bursts(renderer, burst_info, cx, mid_y)
