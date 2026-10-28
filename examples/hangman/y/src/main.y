module main

from hangman import setup.game_rules, setup.parse_word_list, console_play.play, console_play.play_error


--------------------------------------------------------------------------------------------------------
Plays one game of Hangman at the terminal.

system: The root capability, from which the console, the random source and the right to wait come, as system.

fails play_error.input_closed: The terminal closed before the game ended.
fails play_error.console: The terminal could not be read or written.
--------------------------------------------------------------------------------------------------------
function main() fails play_error
        dependency system:
    words = compile parse_word_list(game_rules.word_list)

    inject:
            console = system.console()
            random = system.random()
            forever = system.forever()
        play(words)?
