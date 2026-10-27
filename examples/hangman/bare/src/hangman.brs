//! Lowered from `y/src/hangman.y`, the Y module `hangman`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested.

use std::collections::BTreeSet;
use std::fmt;
use y_runtime::text::{count, join_runes, runes, single_rune};
use y_runtime::{invariant, require, ConsoleError, ConstraintViolation, Console, Forever, Random, Rune};

#[cfg(test)]
mod tests;

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
    let words = Vec::new();
    for piece in text.split(" ") {
        if piece.is_empty() { // y: #blank
            continue;
        }
        match Word::new(piece.to_string()) {
            Ok(parsed) => words.push(parsed),
            Err(_) => return Err(WordListError::NotAWord),
        }
    }
    if words.is_empty() { // y: #empty
        return Err(WordListError::Empty);
    }
    Ok(words)
}

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
        return Outcome::Lost;
    }
    if runes(game.word.as_text()).all(|letter| game.hits.contains(&letter)) { // y: #solved
        return Outcome::Won;
    }
    Outcome::Playing
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
    if trimmed.is_empty() { // y: #nothing_typed
        return Err(GuessError::Empty);
    }
    let letter = single_rune(&trimmed.to_lowercase());
    let Some(letter) = letter else { // y: #not_one_rune, which narrows `letter` to a `Rune`
        return Err(GuessError::NotOneLetter);
    };
    if !letter.is_ascii_lowercase() { // y: #not_a_letter
        return Err(GuessError::NotOneLetter);
    }
    if game.hits.contains(&letter) || game.misses.contains(&letter) { // y: #repeated
        return Err(GuessError::AlreadyGuessed);
    }
    if runes(game.word.as_text()).any(|each| each == letter) { // y: #in_word
        game.hits.insert(letter);
        return Ok(Verdict::Hit);
    }
    game.misses.push(letter);
    invariant(game.misses.len() <= game_rules::ALLOWED_MISSES, "hangman::Game#within_allowance");
    Ok(Verdict::Miss)
}

/// Shows one letter of the word: the letter once it is found, and an underscore until then.
///
/// - `letter`: the letter of the word.
/// - `hits`: the letters found so far.
/// - Returns what the player sees in its place.
fn shown_rune(letter, hits) {
    if hits.contains(&letter) { // y: #known
        return letter;
    }
    Rune('_')
}

/// Shows the word as the player sees it.
///
/// - `game`: the game.
/// - Returns each letter of the word, found or not, separated by spaces: `_ o _ e`.
fn masked(game) {
    let shown = runes(game.word.as_text()).map(|letter| shown_rune(letter, &game.hits)).collect();
    join_runes(&shown, " ")
}

/// Draws the gallows, with one more part of the man for each miss.
///
/// - `misses`: how many wrong guesses the player has made.
/// - Returns the picture, seven lines high.
fn gallows(misses) {
    require(misses <= game_rules::ALLOWED_MISSES, "hangman::gallows#within_allowance");
    let head = " ";
    if misses >= 1 { // y: #has_head
        head = "O";
    }
    let body = " ";
    if misses >= 2 { // y: #has_body
        body = "|";
    }
    let left_arm = " ";
    if misses >= 3 { // y: #has_left_arm
        left_arm = "/";
    }
    let right_arm = " ";
    if misses >= 4 { // y: #has_right_arm
        right_arm = "\\";
    }
    let left_leg = " ";
    if misses >= 5 { // y: #has_left_leg
        left_leg = "/";
    }
    let right_leg = " ";
    if misses >= 6 { // y: #has_right_leg
        right_leg = "\\";
    }
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
/// - `console`, `random`, `forever`: the capabilities of the Y `with` clause.
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
        let Some(typed) = typed else { // y: #closed, which narrows `typed` to a `Text`
            return Err(PlayError::InputClosed);
        };
        match guess(&mut game, &typed) {
            Ok(Verdict::Hit) => console.write_line("Yes.")?,
            Ok(Verdict::Miss) => console.write_line("No.")?,
            Err(GuessError::Empty) => console.write_line("Type a letter.")?,
            Err(GuessError::NotOneLetter) => console.write_line(&format!("\"{typed}\" is not one letter."))?,
            Err(GuessError::AlreadyGuessed) => console.write_line(&format!("You have already guessed {typed}."))?,
        }
    }
    let final_picture = gallows(game.misses.len());
    console.write_line(&final_picture)?;
    if outcome_of(&game) == Outcome::Won { // y: #won
        console.write_line(&format!("You win! The word was {}.", game.word))?;
    } else {
        console.write_line(&format!("Hanged! The word was {}.", game.word))?;
    }
    Ok(())
}
