module hangman

from standard.console use console, console_error
from standard.random use random
from standard.wait use forever

-- The rules every game is played by.
public constants game_rules:
    -- The words a game chooses from, separated by spaces.
    word_list is string = "gallows noose scaffold hangman trapdoor verdict alphabet puzzle riddle lantern"
    -- How many wrong guesses a player may make. The last of them hangs the man.
    allowed_misses is count<element> = 6

------------------------------------------------------------------------------------------------------------------------
A word a game can be played with.

why: A player must be able to type and see every letter, and a short word is too easy to guess.
------------------------------------------------------------------------------------------------------------------------
public type word is string
    where letters_only = value.runes().all(letter => letter.is_ascii_lowercase()) and
          long_enough = (value.count() >= 4)

-- Why a word list cannot be played with.
public enumeration word_list_error:
    empty
    not_a_word

------------------------------------------------------------------------------------------------------------------------
Reads a word list.

text: The words, separated by spaces, as string.
words: Every word of the list, in order, as list<word>.
fails empty: The list has no words.
fails not_a_word: A piece of the list is not a `word`: it has a character other than `a` to `z`, or fewer than four.
------------------------------------------------------------------------------------------------------------------------
public function parse_word_list(text) returns words fails word_list_error:
    words = list.new()
    for piece in text.split(" "):
        if blank = (piece.is_empty()):
            continue
        match word.new(piece.to_string()):
            .ok(parsed) => words.push(parsed)
            .err(_) => fail .not_a_word
    if empty = (words.is_empty()):
        fail .empty
    return words

-- One game: the secret word, and every letter guessed so far.
public record game:
    word: word
    hits: set<rune>
    misses: list<rune>

    invariant within_allowance = (misses.count() <= game_rules.allowed_misses)

-- Where a game stands.
public enumeration outcome:
    playing
    won
    lost

-- What a guess did.
public enumeration verdict:
    hit
    miss

-- Why what the player typed is not a guess.
public enumeration guess_error:
    empty
    not_one_letter
    already_guessed

------------------------------------------------------------------------------------------------------------------------
Starts a game.

word: The word the player must find, as word.
started: A game with nothing guessed, as game.
------------------------------------------------------------------------------------------------------------------------
public function new_game(word) returns started:
    return started = game(word: word, hits: set.new(), misses: list.new())

------------------------------------------------------------------------------------------------------------------------
Says whether a game is won, lost or still going.

game: The game to judge, as game.
standing: Lost once the player has used up the misses allowed, won once every letter of the word is found, and
  playing otherwise, as outcome.
------------------------------------------------------------------------------------------------------------------------
public function outcome_of(game) returns standing:
    if hanged = (game.misses.count() == game_rules.allowed_misses):
        return standing = .lost
    if solved = (game.word.runes().all(letter => game.hits.contains(letter))):
        return standing = .won
    return standing = .playing

------------------------------------------------------------------------------------------------------------------------
Takes one letter from what the player typed, and records it as a hit or a miss.

game: The game, which gains the letter, as game.
line: What the player typed, without its line ending, as string.
result: Whether the letter is in the word, as verdict.
fails empty: The player typed nothing but spaces.
fails not_one_letter: The player typed more than one character, or one that is not a letter.
fails already_guessed: The letter was guessed before. The game is unchanged.
------------------------------------------------------------------------------------------------------------------------
public function guess(mutable game, line) returns result fails guess_error
        requires still_playing = (outcome_of(game) == .playing):
    let trimmed = line.trim()
    if nothing_typed = (trimmed.is_empty()):
        fail .empty
    let letter = trimmed.to_lowercase().single_rune()
    if not_one_rune = (letter == .none):
        fail .not_one_letter
    -- From here `letter` is a `rune`: `not_one_rune` narrows it.
    if not_a_letter = (not letter.is_ascii_lowercase()):
        fail .not_one_letter
    if repeated = (game.hits.contains(letter) or game.misses.contains(letter)):
        fail .already_guessed
    if in_word = (game.word.runes().contains(letter)):
        game.hits.insert(letter)
        return result = .hit
    game.misses.push(letter)
    return result = .miss

------------------------------------------------------------------------------------------------------------------------
Shows one letter of the word: the letter once it is found, and an underscore until then.

letter: The letter of the word, as rune.
hits: The letters found so far, as set<rune>.
shown: What the player sees in its place, as rune.
------------------------------------------------------------------------------------------------------------------------
function shown_rune(letter, hits) returns shown:
    if known = (hits.contains(letter)):
        return shown = letter
    return shown = '_'

------------------------------------------------------------------------------------------------------------------------
Shows the word as the player sees it.

game: The game, as game.
shown: Each letter of the word, found or not, separated by spaces (`_ o _ e`), as string.
------------------------------------------------------------------------------------------------------------------------
public function masked(game) returns shown:
    return shown = game.word.runes().map(letter => shown_rune(letter, game.hits)).join(separator: " ")

------------------------------------------------------------------------------------------------------------------------
Draws the gallows, with one more part of the man for each miss.

misses: How many wrong guesses the player has made, as count<element>.
picture: The drawing, seven lines high, as string.
------------------------------------------------------------------------------------------------------------------------
public function gallows(misses) returns picture
        requires within_allowance = (misses <= game_rules.allowed_misses):
    variable head = " "
    if has_head = (misses >= 1):
        head = "O"
    variable body = " "
    if has_body = (misses >= 2):
        body = "|"
    variable left_arm = " "
    if has_left_arm = (misses >= 3):
        left_arm = "/"
    variable right_arm = " "
    if has_right_arm = (misses >= 4):
        right_arm = "\\"
    variable left_leg = " "
    if has_left_leg = (misses >= 5):
        left_leg = "/"
    variable right_leg = " "
    if has_right_leg = (misses >= 6):
        right_leg = "\\"
    let rows = [
        "  +---+",
        "  |   |",
        "  {head}   |",
        " {left_arm}{body}{right_arm}  |",
        " {left_leg} {right_leg}  |",
        "      |",
        "=========",
    ]
    return picture = rows.join(separator: "\n")

-- Why a game could not be played to its end.
public type play_error:
    case input_closed
    case console(error: console_error)

------------------------------------------------------------------------------------------------------------------------
Plays one game at the console, with a word chosen at random, until it is won or lost.

words: The words to choose from, as list<word>.
console: Where the game is shown and the guesses are read, as console.
random: What chooses the word, as random.
forever: The right to wait for the player as long as they take, as forever.
fails input_closed: The console closed before the game ended.
fails console: The console could not be read or written.
------------------------------------------------------------------------------------------------------------------------
public function play(words) fails play_error
        with console, random, forever
        requires has_words = (not words.is_empty()):
    let chosen = random.index_below(words.count())
    variable game = new_game(words[chosen])
    console.write_line("Hangman: find the word, one letter at a time.")?
    while playing = (outcome_of(game) == .playing):
        let picture = gallows(game.misses.count())
        let shown = masked(game)
        let missed = game.misses.join(separator: " ")
        console.write_line(picture)?
        console.write_line("Word:   {shown}")?
        console.write_line("Missed: {missed}")?
        console.write("Guess:  ")?
        let typed = (console.read_line() forever because "a player may think for as long as they like")?
        if closed = (typed == .none):
            fail .input_closed
        match guess(mutable game, typed):
            .ok(.hit) => console.write_line("Yes.")?
            .ok(.miss) => console.write_line("No.")?
            .err(.empty) => console.write_line("Type a letter.")?
            .err(.not_one_letter) => console.write_line("\"{typed}\" is not one letter.")?
            .err(.already_guessed) => console.write_line("You have already guessed {typed}.")?
    let final_picture = gallows(game.misses.count())
    console.write_line(final_picture)?
    if won = (outcome_of(game) == .won):
        console.write_line("You win! The word was {game.word}.")?
    else:
        console.write_line("Hanged! The word was {game.word}.")?
