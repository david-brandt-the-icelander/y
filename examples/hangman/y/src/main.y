module main

use hangman.{parse_word_list, play, PlayError, WORD_LIST}

---
Plays one game of Hangman at the terminal.

returns: Nothing. The game shows the player how it ended.
fails InputClosed: The terminal closed before the game ended.
fails Console: The terminal could not be read or written.
---
public function main() -> Void ! PlayError
    with system: System
{
    let words = compile parse_word_list(WORD_LIST)
    with console = system.console(), random = system.random(), forever = system.forever() {
        play(words)?
    }
}
