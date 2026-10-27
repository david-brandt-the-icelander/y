//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use hangman_example::hangman::{play, PlayError, Word};
use std::sync::LazyLock;
use y_runtime::System;

/// The value of `compile parse_word_list(game_rules.word_list)` in `main`, which the Y compiler evaluated while building
/// (Y § 3.17): ten words, as data.
const COMPILED_WORD_LIST = ["gallows", "noose", "scaffold", "hangman", "trapdoor", "verdict", "alphabet", "puzzle", "riddle", "lantern"];

/// The compiled words as values, built once, on first use, from `COMPILED_WORD_LIST`.
static WORDS = LazyLock::new(|| COMPILED_WORD_LIST.iter().map(|word| Word::new(word.to_string()).expect("y: a compiled word is a Word")).collect());

/// Plays one game of Hangman at the terminal.
///
/// - Returns nothing. The game shows the player how it ended.
/// - Fails `InputClosed`: the terminal closed before the game ended.
/// - Fails `Console`: the terminal could not be read or written.
fn main() {
    let system = System::root();
    let words = &*WORDS; // y: compile parse_word_list(game_rules.word_list)
    {
        // y: with console = system.console(), random = system.random(), forever = system.forever()
        let console = system.console();
        let random = system.random();
        let forever = system.forever();
        play(words, &mut console, &mut random, &forever)?;
    }
    Ok(())
}

/// Generated for every `compile`: the run-time call gives the value the compiler wrote down (Y § 3.17).
#[cfg(test)]
mod tests {
    use super::*;
    use hangman_example::hangman::{game_rules, parse_word_list};

    #[test]
    fn compiled_word_list_matches_run_time() {
        let run_time = parse_word_list(game_rules::WORD_LIST);
        assert!(run_time == Ok(WORDS.clone()), "y: compile parse_word_list(game_rules.word_list)");
    }
}
