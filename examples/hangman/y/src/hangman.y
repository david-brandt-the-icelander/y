module hangman

use standard.console.{Console, ConsoleError}
use standard.random.Random
use standard.wait.Forever

-- The words a game chooses from, separated by spaces.
public constant WORD_LIST: Text = "gallows noose scaffold hangman trapdoor verdict alphabet puzzle riddle lantern"

-- How many wrong guesses a player may make. The last of them hangs the man.
public constant ALLOWED_MISSES: Count<Element> = 6

---
A word a game can be played with.

why: A player must be able to type and see every letter, and a short word is too easy to guess.
---
public type Word = Text
    where letters_only = value.runes().all(rune => rune.is_ascii_lowercase()),
          long_enough = (value.count() >= 4)

-- Why a word list cannot be played with.
public type WordListError {
    case Empty
    case NotAWord(text: Text)
}

---
Reads a word list.

text: The words, separated by spaces.
returns: Every word of the list, in order.
fails Empty: The list has no words.
fails NotAWord: A piece of the list is not a `Word`: it has a character other than `a` to `z`, or fewer than four.
---
public function parse_word_list(text: &Text) -> List<Word> ! WordListError {
    variable words = List.new()
    for piece in text.split(" ") {
        if blank = (piece.is_empty()) {
            continue
        }
        match Word.new(piece.to_text()) {
            .Ok(word) => words.push(move word),
            .Err(_) => fail .NotAWord(text: piece.to_text()),
        }
    }
    if empty = (words.is_empty()) {
        fail .Empty
    }
    words
}

-- One game: the secret word, and every letter guessed so far.
public record Game {
    word: Word,
    hits: Set<Rune>,
    misses: List<Rune>,

    invariant within_allowance = (misses.count() <= ALLOWED_MISSES)
}

-- Where a game stands.
public type Outcome {
    case Playing
    case Won
    case Lost
}

-- What a guess did.
public type Verdict {
    case Hit
    case Miss
}

-- Why what the player typed is not a guess.
public type GuessError {
    case Empty
    case NotOneLetter(text: Text)
    case AlreadyGuessed(letter: Rune)
}

---
Starts a game.

word: The word the player must find.
returns: A game with nothing guessed.
---
public function new_game(word: Word) -> Game {
    Game { word: move word, hits: Set.new(), misses: List.new() }
}

---
Says whether a game is won, lost or still going.

game: The game to judge.
returns: Lost once the player has used up the misses allowed, Won once every letter of the word is found, and
  Playing otherwise.
---
public function outcome(game: &Game) -> Outcome {
    if hanged = (game.misses.count() == ALLOWED_MISSES) {
        return .Lost
    }
    if solved = (game.word.runes().all(rune => game.hits.contains(rune))) {
        return .Won
    }
    .Playing
}

---
Takes one letter from what the player typed, and records it as a hit or a miss.

game: The game, which gains the letter.
line: What the player typed, without its line ending.
returns: Whether the letter is in the word.
fails Empty: The player typed nothing but spaces.
fails NotOneLetter: The player typed more than one character, or one that is not a letter.
fails AlreadyGuessed: The letter was guessed before. The game is unchanged.
---
public function guess(game: &mutable Game, line: &Text) -> Verdict ! GuessError
    requires still_playing = (outcome(game) == .Playing)
{
    let trimmed = line.trim()
    if nothing_typed = (trimmed.is_empty()) {
        fail .Empty
    }
    let letter = trimmed.to_lowercase().single_rune()
    if not_one_rune = (letter == .None) {
        fail .NotOneLetter(text: trimmed.to_text())
    }
    -- From here `letter` is a `Rune`: `not_one_rune` narrows it.
    if not_a_letter = (not letter.is_ascii_lowercase()) {
        fail .NotOneLetter(text: trimmed.to_text())
    }
    if repeated = (game.hits.contains(letter) or game.misses.contains(letter)) {
        fail .AlreadyGuessed(letter: letter)
    }
    if in_word = (game.word.runes().contains(letter)) {
        game.hits.insert(letter)
        return .Hit
    }
    game.misses.push(letter)
    .Miss
}

---
Shows one letter of the word: the letter once it is found, and an underscore until then.

rune: The letter of the word.
hits: The letters found so far.
returns: What the player sees in its place.
---
function shown_rune(rune: Rune, hits: &Set<Rune>) -> Rune {
    if known = (hits.contains(rune)) {
        return rune
    }
    '_'
}

---
Shows the word as the player sees it.

game: The game.
returns: Each letter of the word, found or not, separated by spaces: `_ o _ e`.
---
public function masked(game: &Game) -> Text {
    game.word.runes().map(rune => shown_rune(rune, &game.hits)).join(separator: " ")
}

---
Draws the gallows, with one more part of the man for each miss.

misses: How many wrong guesses the player has made.
returns: The picture, seven lines high.
---
public function gallows(misses: Count<Element>) -> Text
    requires within_allowance = (misses <= ALLOWED_MISSES)
{
    variable head = " "
    if has_head = (misses >= 1) {
        head = "O"
    }
    variable body = " "
    if has_body = (misses >= 2) {
        body = "|"
    }
    variable left_arm = " "
    if has_left_arm = (misses >= 3) {
        left_arm = "/"
    }
    variable right_arm = " "
    if has_right_arm = (misses >= 4) {
        right_arm = "\\"
    }
    variable left_leg = " "
    if has_left_leg = (misses >= 5) {
        left_leg = "/"
    }
    variable right_leg = " "
    if has_right_leg = (misses >= 6) {
        right_leg = "\\"
    }
    let rows = [
        "  +---+",
        "  |   |",
        "  {head}   |",
        " {left_arm}{body}{right_arm}  |",
        " {left_leg} {right_leg}  |",
        "      |",
        "=========",
    ]
    rows.join(separator: "\n")
}

-- Why a game could not be played to its end.
public type PlayError {
    case InputClosed
    case Console(error: ConsoleError)
}

---
Plays one game at the console, with a word chosen at random, until it is won or lost.

words: The words to choose from.
returns: Nothing. The console shows the player how the game ended.
fails InputClosed: The console closed before the game ended.
fails Console: The console could not be read or written.
---
public function play(words: &List<Word>) -> Void ! PlayError
    with console: Console, random: Random, forever: Forever
    requires has_words = (not words.is_empty())
{
    let chosen = random.index_below(words.count())
    variable game = new_game(words[chosen].clone())
    console.write_line("Hangman: find the word, one letter at a time.")?
    while playing = (outcome(&game) == .Playing) {
        let picture = gallows(game.misses.count())
        let shown = masked(&game)
        let missed = game.misses.join(separator: " ")
        console.write_line(&picture)?
        console.write_line("Word:   {shown}")?
        console.write_line("Missed: {missed}")?
        console.write("Guess:  ")?
        let typed = (console.read_line() forever because "a player may think for as long as they like")?
        if closed = (typed == .None) {
            fail .InputClosed
        }
        match guess(&mutable game, &typed) {
            .Ok(.Hit) => console.write_line("Yes.")?,
            .Ok(.Miss) => console.write_line("No.")?,
            .Err(.Empty) => console.write_line("Type a letter.")?,
            .Err(.NotOneLetter(text)) => console.write_line("\"{text}\" is not one letter.")?,
            .Err(.AlreadyGuessed(letter)) => console.write_line("You have already guessed {letter}.")?,
        }
    }
    let final_picture = gallows(game.misses.count())
    console.write_line(&final_picture)?
    if won = (outcome(&game) == .Won) {
        console.write_line("You win! The word was {game.word}.")?
    } else {
        console.write_line("Hanged! The word was {game.word}.")?
    }
}
