module main

from nim import console_play.play, console_play.play_error


--------------------------------------------------------------------
Plays one game of Nim at the terminal.

system: The root capability, from which the console and the right to wait come, as system.

fails play_error.input_closed: The terminal closed before the game ended.
fails play_error.console: The terminal could not be read or written.
--------------------------------------------------------------------
function main(injected system) fails play_error:
    inject(console = system.console(), forever = system.forever()):
        play() or fail
