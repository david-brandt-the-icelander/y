module main

from snake import drawing.layout, window_play.play
from standard.console import console_error


------------------------------------------------------------------------------------------------
Plays Snake in a window, and when the window is closed, writes the best score at the terminal.

system: The root capability, from which the window, the clock, the randomness, the right to wait
  and the console come, as system.

fails console_error: The terminal could not be written.
------------------------------------------------------------------------------------------------
function main(injected system) fails console_error:
    console = system.console()

    inject(
            window = system.window(title: "Snake", width: layout.width, height: layout.height),
            clock = system.clock(),
            random = system.random(),
            forever = system.forever()):
        best = play()
        console.write_line("Your best score: {best}.") or fail
