module main

from hangman import game_rules, parse_word_list, play, play_error


------------------------------------------------------------------------------------------------------------------------
Plays one game of Hangman at the terminal.

system: The root capability, from which the console, the random source and the right to wait come, as system.
fails play_error.input_closed: The terminal closed before the game ended.
fails play_error.console: The terminal could not be read or written.
------------------------------------------------------------------------------------------------------------------------
public function main() fails play_error
        with system:
    let words = compile parse_word_list(game_rules.word_list)
    with console = system.console(), random = system.random(), forever = system.forever():
        play(words)?
