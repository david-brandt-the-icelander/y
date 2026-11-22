module main

from memory import drawing.layout, setup.openmoji, setup.setup_error, window_play.play
from standard.console import console_error


-- Why the program ended early.
type main_error:
    case setup(error is setup_error)
    case console(error is console_error)


----------------------------------------------------------------------------------------------------
Plays Memory in a window, and when the window is closed, writes the best score at the terminal.

system: The root capability, from which the window, the clock, the randomness, the website, the right to wait and the
  console come, as system.

fails main_error.setup: A game could not be set up, because its pictures could not be fetched.
fails main_error.console: The terminal could not be written.
----------------------------------------------------------------------------------------------------
function main(injected system) fails main_error:
    console = system.console()

    inject(
            window = system.window(title: "Memory", width: layout.width, height: layout.height),
            clock = system.clock(),
            random = system.random(),
            site = system.http(openmoji),
            forever = system.forever()):
        best = play() or fail
        console.write_line("Your best score: {best}.") or fail
