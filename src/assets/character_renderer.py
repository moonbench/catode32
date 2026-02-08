from assets.character import CHAR_IMG_HEAD1, CHAR_IMG_BODY1, CHAR_IMG_EYES1, CHAR_IMG_TAIL1

class CharacterRenderer():
    def __init__(self, renderer):
        self.renderer = renderer

    def draw_character(self, context, x, y):
        # Body
        body = CHAR_IMG_BODY1
        body_x = x - body["anchor_x"]
        body_y = y - body["anchor_y"]
        self.renderer.draw_sprite_obj(body, body_x, body_y)

        # Head
        head = CHAR_IMG_HEAD1
        head_root_x = body_x + body["head_x"]
        head_root_y = body_y + body["head_y"]

        head_x = head_root_x - head["anchor_x"]
        head_y = head_root_y - head["anchor_y"]
        self.renderer.draw_sprite_obj(head, head_x, head_y)

        # Eyes
        eyes = CHAR_IMG_EYES1
        eye_frame = 0
        if context["blink"] < 0.9:
            eye_frame = 1 if (context["blink"] < 0.3 or context["blink"] > 0.6) else 2
        eye_x = head_x + head["eye_x"] - eyes["anchor_x"]
        eye_y = head_y + head["eye_y"] - eyes["anchor_y"]
        self.renderer.draw_sprite_obj(eyes, eye_x, eye_y, frame=eye_frame)

        # Tail
        tail = CHAR_IMG_TAIL1
        tail_frame = int(context["tail"]) % 16
        tail_root_x = body_x + body["tail_x"]
        tail_root_y = body_y + body["tail_y"]
        tail_x = tail_root_x - tail["anchor_x"]
        tail_y = tail_root_y - tail["anchor_y"]
        self.renderer.draw_sprite_obj(tail, tail_x, tail_y, frame=tail_frame)





