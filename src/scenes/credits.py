"""
Credits scene - scrollable credits text.
Up/Down to scroll, B or Menu to exit.
"""
from scene import Scene
from ui import Popup

CREDITS = """\
===============
Catode32 Credit
---------------

Thanks for
playing!

---------------

Code & Design:
Moonbench

---------------

This pet was
inspired by my
two wonderful
cats.

My sweet and
intelligent
tortoiseshell
girl, Bean, and
her adopted
rambunctious
black sister,
Juno. They are
both full of
so much love.

I started to
create the art
for this project
shortly after
adopting Bean,
and was finally
inspired to
make the toy
into reality
after adopting
Juno.

Their great
personalities
inspired the
cat in this toy
and it wouldn't
exist without
them! I took
many breaks
while working
on this to give
them lots of
love and
attention!

---------------

I also want
to give thanks
to my friends
and family who
helped test
this and who
provided warm
encouragment
and support.

---------------

The source code
for this toy is
available. Just
search for
"catode32"
online!

---------------

I also want to
give thanks to
the open-source
community for
the projects
that this was
built upon.

---------------

Oh, and if you
have the time
and space, then
please consider
adopting a real
pet from your
local animal
shelter, and
give them lots
of love too!

---------------


Made with love!

|\          /|
| \________/ |
|            |
|  /\    /\  |
 \   ,__,   /
  \________/
\
"""


class CreditsScene(Scene):
    MODULES_TO_KEEP = []

    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.popup = None

    def load(self):
        super().load()
        # Full-screen popup: 15 chars/line, 7 visible lines
        self.popup = Popup(self.renderer, x=0, y=0, width=128, height=64, padding=4)
        self.popup.set_text(CREDITS, wrap=False)

    def unload(self):
        super().unload()

    def enter(self):
        self.popup.scroll_offset = 0

    def handle_input(self):
        inp = self.input
        if inp.was_just_pressed('b') or inp.was_just_pressed('menu2'):
            return ('change_scene', 'last_main')
        if inp.was_just_pressed('up'):
            self.popup.scroll_up()
        elif inp.was_just_pressed('down'):
            self.popup.scroll_down()

    def update(self, dt):
        pass

    def draw(self):
        self.popup.draw(show_scroll_indicators=True)
        self.renderer.show()
