"""
Credits scene - scrollable credits text.
Up/Down to scroll, B or Menu to exit.
"""
from scene import Scene
from ui import Popup

YEAR = "2026"
VERSION = "0.0.2"

CREDITS = """\
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
inspired by my
two wonderful
cats. My sweet
and very
intelligent
tortoiseshell
girl, Bean,
and her cute,
rambunctious,
black sister,
Juno.

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

Their sweet
personalities
inspired the
pet in this
virtual toy.
It wouldn't
exist without
them and their
love.

_______________

I want to give
thanks to my
friends and
family who
helped test
this and who
provided their
encouragment
and support.
_______________

I also want to
give thanks to
the open-source
community for
the projects
that this was
built upon.

In that spirit,
the source code
for this pet is
available. Just
search for
"catode32".
_______________

If you have the
time and space,
then I hope you
will consider
adopting a real
pet from your
local animal
shelter, and
give them lots
of love too!
_______________


|\          /|
| \________/ |
|            |
|  /\    /\  |
==          ==
 \   ,__,   /
  \________/

===============

""" + f"Catode32 - {YEAR}\nv{VERSION}"


class CreditsScene(Scene):

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
