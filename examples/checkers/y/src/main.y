module main

from checkers import drawing.layout, window_play.play
from standard.console import console_error


----------------------------------------------------------------------------------------------------
Plays Checkers in a window, and when the window is closed, writes the best score at the terminal.

system: The root capability, from which the window, the randomness, the right to wait and the console come, as
  system.

fails console_error: The terminal could not be written.
----------------------------------------------------------------------------------------------------
function main(injected system) fails console_error:
    console = system.console()

    inject(
            window = system.window(title: "Checkers", width: layout.width, height: layout.height),
            random = system.random(),
            forever = system.forever()):
        best = play()
        console.write_line("Your best score: {best}.") or fail
