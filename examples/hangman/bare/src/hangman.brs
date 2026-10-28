//! Lowered from `y/src/hangman.y`, the Y module `hangman`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `hangman::setup::parse_word_list`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `switch`
//! arm on its `match` arm.

use std::collections::BTreeSet;
use std::fmt;
use y_runtime::text::{count, join_runes, runes, single_rune};
use y_runtime::{invariant, require, ConsoleError, ConstraintViolation, Console, Forever, Random, Rune};
use self::{display::*, rounds::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the words a game can be played with, and the rules it is played by.
mod setup {
    use super::*;

    /// The rules every game is played by: the Y constant group `game_rules`.
    mod game_rules {
        /// The words a game chooses from, separated by spaces.
        const WORD_LIST = "gallows noose scaffold hangman trapdoor verdict alphabet puzzle riddle lantern";

        /// How many wrong guesses a player may make. The last of them hangs the man.
        const ALLOWED_MISSES = 6;
    }

    /// A word a game can be played with.
    ///
    /// Why: a player must be able to type and see every letter, and a short word is too easy to guess.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Word(_);

    impl Word {
        /// Checks both constraints of `Word`, in the order the Y source names them.
        fn new(value) {
            if !runes(&value).all(|letter| letter.is_ascii_lowercase()) { // y: Word#letters_only
                return Err(ConstraintViolation { constraint: "hangman::Word#letters_only" });
            }
            if !(count(&value) >= 4) { // y: Word#long_enough
                return Err(ConstraintViolation { constraint: "hangman::Word#long_enough" });
            }
            Ok(Word(value))
        }

        /// The word as text.
        fn as_text(self) {
            &self.0
        }
    }

    impl fmt::Display for Word {
        fn fmt(self, f) {
            f.write_str(&self.0)
        }
    }

    /// Why a word list cannot be played with.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum WordListError {
        Empty,
        NotAWord,
    }

    /// Reads a word list.
    ///
    /// - `text`: the words, separated by spaces.
    /// - Returns every word of the list, in order.
    /// - Fails `Empty`: the list has no words.
    /// - Fails `NotAWord`: a piece of the list is not a `Word`: it has a character other than `a` to `z`, or fewer than four.
    fn parse_word_list(text) {
        let found = Vec::new();
        for piece in text.split(" ") {
            if piece.is_empty() { // y: #blank, else #has_text
                continue;
            }
            match Word::new(piece.to_string()) {
                Ok(parsed) => found.push(parsed), // y: #valid
                Err(_) => return Err(WordListError::NotAWord), // y: #invalid
            }
        }
        if found.is_empty() { // y: #empty, else #has_words
            return Err(WordListError::Empty);
        }
        Ok(found)
    }
}

/// The Y facet `rounds`: one game, and what a guess does to it.
mod rounds {
    use super::*;

    /// One game: the secret word, and every letter guessed so far.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Game {
        word,
        hits,
        misses,
    }

    /// Where a game stands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Playing,
        Won,
        Lost,
    }

    /// What a guess did.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Verdict {
        Hit,
        Miss,
    }

    /// Why what the player typed is not a guess.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum GuessError {
        Empty,
        NotOneLetter,
        AlreadyGuessed,
    }

    /// Starts a game.
    ///
    /// - `word`: the word the player must find.
    /// - Returns a game with nothing guessed.
    fn new_game(word) {
        Game { word, hits: BTreeSet::new(), misses: Vec::new() }
    }

    /// Says whether a game is won, lost or still going.
    ///
    /// - `game`: the game to judge.
    /// - Returns `Lost` once the player has used up the misses allowed, `Won` once every letter of the word is found, and
    ///   `Playing` otherwise.
    fn outcome_of(game) {
        if game.misses.len() == game_rules::ALLOWED_MISSES { // y: #hanged
            Outcome::Lost
        } else if runes(game.word.as_text()).all(|letter| game.hits.contains(&letter)) { // y: #solved
            Outcome::Won
        } else { // y: #undecided
            Outcome::Playing
        }
    }

    /// Takes one letter from what the player typed, and records it as a hit or a miss.
    ///
    /// - `game`: the game, which gains the letter.
    /// - `line`: what the player typed, without its line ending.
    /// - Returns whether the letter is in the word.
    /// - Fails `Empty`: the player typed nothing but spaces.
    /// - Fails `NotOneLetter`: the player typed more than one character, or one that is not a letter.
    /// - Fails `AlreadyGuessed`: the letter was guessed before. The game is unchanged.
    fn guess(game, line) {
        require(outcome_of(game) == Outcome::Playing, "hangman::guess#still_playing");
        let trimmed = line.trim();
        let letter = single_rune(&trimmed.to_lowercase());
        // y: the Y source's one `if`-chain; its first two branches fail, so the narrowing of `letter` is a let-else here
        if trimmed.is_empty() { // y: #nothing_typed
            return Err(GuessError::Empty);
        }
        let Some(letter) = letter else { // y: #not_one_rune, which narrows `letter` to a `rune`
            return Err(GuessError::NotOneLetter);
        };
        if !letter.is_ascii_lowercase() { // y: #not_a_letter
            Err(GuessError::NotOneLetter)
        } else if game.hits.contains(&letter) || game.misses.contains(&letter) { // y: #repeated
            Err(GuessError::AlreadyGuessed)
        } else if runes(game.word.as_text()).any(|each| each == letter) { // y: #in_word
            game.hits.insert(letter);
            Ok(Verdict::Hit)
        } else { // y: #not_in_word
            game.misses.push(letter);
            invariant(game.misses.len() <= game_rules::ALLOWED_MISSES, "hangman::Game#within_allowance");
            Ok(Verdict::Miss)
        }
    }
}

/// The Y facet `display`: what the player sees of a game.
mod display {
    use super::*;

    /// Shows the word as the player sees it.
    ///
    /// - `game`: the game.
    /// - Returns each letter of the word, found or not, separated by spaces: `_ o _ e`.
    fn masked(game) {
        let letters = Vec::new();
        for letter in runes(game.word.as_text()) {
            letters.push(shown_rune(letter, &game.hits));
        }
        join_runes(&letters, " ")
    }

    /// Draws the gallows, with one more part of the man for each miss.
    ///
    /// - `misses`: how many wrong guesses the player has made.
    /// - Returns the picture, seven lines high.
    fn gallows(misses) {
        require(misses <= game_rules::ALLOWED_MISSES, "hangman::gallows#within_allowance");
        let head = part(misses, 1, "O");
        let body = part(misses, 2, "|");
        let left_arm = part(misses, 3, "/");
        let right_arm = part(misses, 4, "\\");
        let left_leg = part(misses, 5, "/");
        let right_leg = part(misses, 6, "\\");
        let rows = [
            "  +---+".to_string(),
            "  |   |".to_string(),
            format!("  {head}   |"),
            format!(" {left_arm}{body}{right_arm}  |"),
            format!(" {left_leg} {right_leg}  |"),
            "      |".to_string(),
            "=========".to_string(),
        ];
        rows.join("\n")
    }
}

/// Shows one letter of the word: the letter once it is found, and an underscore until then.
///
/// - `letter`: the letter of the word.
/// - `hits`: the letters found so far.
/// - Returns what the player sees in its place.
fn shown_rune(letter, hits) {
    if hits.contains(&letter) { // y: #known
        letter
    } else { // y: #unknown
        Rune('_')
    }
}

/// One part of the man, drawn once the player has missed often enough.
///
/// - `misses`: how many wrong guesses the player has made.
/// - `needed`: how many misses draw this part.
/// - `mark`: the character that draws it.
/// - Returns the mark, or a space while the part is not drawn yet.
fn part(misses, needed, mark) {
    if misses >= needed { // y: #reached
        mark.to_string()
    } else { // y: #not_yet
        " ".to_string()
    }
}

/// The Y facet `console_play`: playing a whole game at the console.
mod console_play {
    use super::*;

    /// Why a game could not be played to its end.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum PlayError {
        InputClosed,
        Console { error },
    }

    /// `?` on a `ConsoleError` converts through `PlayError`'s one case that carries it (Y § 3.4).
    impl From for PlayError {
        fn from(error) {
            PlayError::Console { error }
        }
    }

    /// Plays one game at the console, with a word chosen at random, until it is won or lost.
    ///
    /// - `words`: the words to choose from.
    /// - `console`, `random`, `forever`: the capabilities in the Y `dependencies:` block.
    /// - Returns nothing. The console shows the player how the game ended.
    /// - Fails `InputClosed`: the console closed before the game ended.
    /// - Fails `Console`: the console could not be read or written.
    fn play(words, console, random, forever) {
        require(!words.is_empty(), "hangman::play#has_words");
        let chosen = random.index_below(words.len());
        let game = new_game(words[chosen].clone()); // y: a copy, because new_game keeps its word and `words` is still read
        console.write_line("Hangman: find the word, one letter at a time.")?;
        while outcome_of(&game) == Outcome::Playing { // y: #playing
            let picture = gallows(game.misses.len());
            let shown = masked(&game);
            let missed = join_runes(&game.misses, " ");
            console.write_line(&picture)?;
            console.write_line(&format!("Word:   {shown}"))?;
            console.write_line(&format!("Missed: {missed}"))?;
            console.write("Guess:  ")?;
            let typed = console.read_line(forever)?; // y: forever because "a player may think for as long as they like"
            let Some(typed) = typed else { // y: #closed, else #line_read, which narrows `typed` to a `string`
                return Err(PlayError::InputClosed);
            };
            match guess(&mut game, &typed) {
                Ok(Verdict::Hit) => console.write_line("Yes.")?, // y: #hit
                Ok(Verdict::Miss) => console.write_line("No.")?, // y: #miss
                Err(GuessError::Empty) => console.write_line("Type a letter.")?, // y: #typed_nothing
                Err(GuessError::NotOneLetter) => console.write_line(&format!("\"{typed}\" is not one letter."))?, // y: #typed_other
                Err(GuessError::AlreadyGuessed) => console.write_line(&format!("You have already guessed {typed}."))?, // y: #typed_again
            }
        }
        // y: #game_over, the `else` of `while playing`: the loop has no `break`, so it runs when the loop ends
        let final_picture = gallows(game.misses.len());
        console.write_line(&final_picture)?;
        if outcome_of(&game) == Outcome::Won { // y: #won
            console.write_line(&format!("You win! The word was {}.", game.word))?;
        } else { // y: #hanged
            console.write_line(&format!("Hanged! The word was {}.", game.word))?;
        }
        Ok(())
    }
}
