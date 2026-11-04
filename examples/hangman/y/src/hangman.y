module hangman

from standard.console import console, console_error
from standard.random import random
from standard.wait import forever


-- The words a game can be played with, and the rules it is played by.
public facet setup:
    -- The rules every game is played by.
    constants game_rules:
        -- The words a game chooses from, separated by spaces.
        word_list is string = "gallows noose scaffold hangman trapdoor verdict alphabet puzzle riddle lantern"

        -- How many wrong guesses a player may make. The last of them hangs the man.
        allowed_misses is count<of=rune> = 6


    ------------------------------------------------------------------------------------------------------------
    A word a game can be played with.

    why: A player must be able to type and see every letter, and a short word is too easy to guess.
    ------------------------------------------------------------------------------------------------------------
    type word is string:
        where letters_only = value.matches(pattern"^[a-z]*$") and
              long_enough = (value.count() >= 4)


    -- Why a word list cannot be played with.
    constants word_list_error:
        empty = "the list has no words"
        not_a_word = "a piece of the list is not a word"


    ------------------------------------------------------------------------------------------------------------
    Reads a word list.

    text: The words, separated by spaces, as string.

    words: Every word of the list, in order, as list<element=word>.

    fails word_list_error.empty: The list has no words.
    fails word_list_error.not_a_word: A piece of the list is not a `word`: it has a character other than `a` to `z`, or
      fewer than four.
    ------------------------------------------------------------------------------------------------------------
    function parse_word_list(text) returns words fails word_list_error:
        mutable found = list.new()

        for piece in text.split(" "):
            if blank = piece.is_empty():
                continue
            else has_text:
                conditional checked = word.new(piece.to_string()):
                    condition checked.valid = (checked == result.ok):
                        found.push(checked.value)
                    condition checked.invalid = (checked == result.err):
                        fail word_list_error.not_a_word

        if empty = found.is_empty():
            fail word_list_error.empty
        else has_words:
            return words = found


-- One game, and what a guess does to it.
public facet rounds:
    -- One game: the secret word, and every letter guessed so far.
    structure game:
        word is word
        hits is set<element=rune>
        misses is list<element=rune>

        derived miss_count is count<of=rune> = misses.count()

        invariant within_allowance = (misses.count() <= game_rules.allowed_misses)


    -- Where a game stands.
    constants outcome:
        playing = "playing"
        won = "won"
        lost = "lost"


    -- What a guess did.
    constants verdict:
        hit = "hit"
        miss = "miss"


    -- Why what the player typed is not a guess.
    constants guess_error:
        empty = "nothing was typed"
        not_one_letter = "not one letter"
        already_guessed = "already guessed"


    ------------------------------------------------------------------------------------------------------------
    Starts a game.

    word: The word the player must find, as word.

    started: A game with nothing guessed, as game.
    ------------------------------------------------------------------------------------------------------------
    function new_game(word) returns started:
        return started = game(word: word, hits: set.new(), misses: list.new())


    ------------------------------------------------------------------------------------------------------------
    The player has used up the misses allowed.

    game: The game to judge, as game.
    ------------------------------------------------------------------------------------------------------------
    rule hanged(game) = (game.miss_count == game_rules.allowed_misses)


    ------------------------------------------------------------------------------------------------------------
    Every letter of the word has been found.

    game: The game to judge, as game.
    ------------------------------------------------------------------------------------------------------------
    rule solved(game) = game.word.runes().to_set().is_subset_of(game.hits)


    ------------------------------------------------------------------------------------------------------------
    Says whether a game is won, lost or still going.

    game: The game to judge, as game.

    standing: Lost once the player has used up the misses allowed, won once every letter of the word is found, and
      playing otherwise, as outcome.
    ------------------------------------------------------------------------------------------------------------
    function outcome_of(game) returns standing:
        conditional judged:
            condition judged.hanged = hanged(game):
                return standing = outcome.lost
            condition judged.solved = solved(game):
                return standing = outcome.won
            else judged.undecided:
                return standing = outcome.playing


    ------------------------------------------------------------------------------------------------------------
    Takes one letter from what the player typed, and records it as a hit or a miss.

    game: The game, which gains the letter, as game.
    line: What the player typed, without its line ending, as string.

    answer: Whether the letter is in the word, as verdict.

    fails guess_error.empty: The player typed nothing but spaces.
    fails guess_error.not_one_letter: The player typed more than one character, or one that is not a letter.
    fails guess_error.already_guessed: The letter was guessed before. The game is unchanged.
    ------------------------------------------------------------------------------------------------------------
    function guess(mutable game, line) returns answer fails guess_error
            where still_playing = (outcome_of(game) == outcome.playing):
        trimmed = line.trim()
        letter = trimmed.to_lowercase().single_rune()

        conditional judged:
            condition judged.nothing_typed = trimmed.is_empty():
                fail guess_error.empty
            condition judged.not_one_rune = (letter == optional.none):
                fail guess_error.not_one_letter

            -- In the conditions below, `letter` is a `rune`: `judged.not_one_rune` narrows it.
            condition judged.not_a_letter = (not letter.is_ascii_lowercase()):
                fail guess_error.not_one_letter
            condition judged.repeated = (game.hits.contains(letter) or game.misses.contains(letter)):
                fail guess_error.already_guessed
            condition judged.in_word = game.word.runes().contains(letter):
                game.hits.insert(letter)
                return answer = verdict.hit
            else judged.not_in_word:
                game.misses.push(letter)
                return answer = verdict.miss


-- What the player sees of a game.
public facet display:
    ------------------------------------------------------------------------------------------------------------
    Shows the word as the player sees it.

    game: The game, as game.

    shown: Each letter of the word, found or not, separated by spaces (`_ o _ e`), as string.
    ------------------------------------------------------------------------------------------------------------
    function masked(game) returns shown:
        mutable letters = list.new()

        for letter in game.word.runes():
            letters.push(shown_rune(letter, game.hits))
        return shown = letters.join(separator: " ")


    ------------------------------------------------------------------------------------------------------------
    Draws the gallows, with one more part of the man for each miss.

    misses: How many wrong guesses the player has made, as count<of=rune>.

    picture: The drawing, seven lines high, as string.
    ------------------------------------------------------------------------------------------------------------
    function gallows(misses) returns picture
            where within_allowance = (misses <= game_rules.allowed_misses):
        head = part(misses: misses, needed: 1, mark: "O")
        body = part(misses: misses, needed: 2, mark: "|")
        left_arm = part(misses: misses, needed: 3, mark: "/")
        right_arm = part(misses: misses, needed: 4, mark: "\\")
        left_leg = part(misses: misses, needed: 5, mark: "/")
        right_leg = part(misses: misses, needed: 6, mark: "\\")
        rows = [
            "  +---+",
            "  |   |",
            "  {head}   |",
            " {left_arm}{body}{right_arm}  |",
            " {left_leg} {right_leg}  |",
            "      |",
            "=========",
        ]
        return picture = rows.join(separator: "\n")


----------------------------------------------------------------------------------------------------------------
Shows one letter of the word: the letter once it is found, and an underscore until then.

letter: The letter of the word, as rune.
hits: The letters found so far, as set<element=rune>.

shown: What the player sees in its place, as rune.
----------------------------------------------------------------------------------------------------------------
function shown_rune(letter, hits) returns shown:
    if known = hits.contains(letter):
        return shown = letter
    else unknown:
        return shown = '_'


----------------------------------------------------------------------------------------------------------------
One part of the man, drawn once the player has missed often enough.

misses: How many wrong guesses the player has made, as count<of=rune>.
needed: How many misses draw this part, as count<of=rune>.
mark: The character that draws it, as string.

drawn: The mark, or a space while the part is not drawn yet, as string.
----------------------------------------------------------------------------------------------------------------
function part(misses, needed, mark) returns drawn:
    if reached = (misses >= needed):
        return drawn = mark
    else not_yet:
        return drawn = " "


-- Playing a whole game at the console.
public facet console_play:
    -- Why a game could not be played to its end.
    type play_error:
        case input_closed
        case console(error is console_error)


    ------------------------------------------------------------------------------------------------------------
    Plays one game at the console, with a word chosen at random, until it is won or lost.

    words: The words to choose from, as list<element=word>.

    console: Where the game is shown and the guesses are read, as console.
    random: What chooses the word, as random.
    forever: The right to wait for the player as long as they take, as forever.

    fails play_error.input_closed: The console closed before the game ended.
    fails play_error.console: The console could not be read or written.
    ------------------------------------------------------------------------------------------------------------
    function play(words, injected console, injected random, injected forever) fails play_error
            where has_words = (not words.is_empty()):
        chosen = random.index_below(words.count())
        mutable game = new_game(words[chosen])
        console.write_line("Hangman: find the word, one letter at a time.") or fail

        while playing = (outcome_of(game) == outcome.playing):
            show_turn(game) or fail
            typed = console.read_line() forever because "a player may think for as long as they like" or fail

            if closed = (typed == optional.none):
                fail play_error.input_closed
            else line_read:
                guessed = guess(mutable game, typed)
                console.write_line(reply(guessed, typed)) or fail
        else over_at_once:
            fail not_reachable
        show_end(game) or fail


----------------------------------------------------------------------------------------------------------------
Shows the gallows, the word as the player sees it and the letters missed, and asks for a guess.

game: The game, as game.

console: Where the game is shown, as console.

fails console_error: The console could not be written.
----------------------------------------------------------------------------------------------------------------
function show_turn(game, injected console) fails console_error:
    picture = gallows(game.miss_count)
    shown = masked(game)
    missed = game.misses.join(separator: " ")
    console.write_line(picture) or fail
    console.write_line("Word:   {shown}") or fail
    console.write_line("Missed: {missed}") or fail
    console.write("Guess:  ") or fail


----------------------------------------------------------------------------------------------------------------
What the game says to a guess.

guessed: What the guess did, as result<value=verdict, failure=guess_error>.
typed: What the player typed, as string.

answer: The line to write, as string.
----------------------------------------------------------------------------------------------------------------
function reply(guessed, typed) returns answer:
    conditional said = guessed:
        condition said.hit = (said == result.ok(verdict.hit)):
            return answer = "Yes."
        condition said.miss = (said == result.ok(verdict.miss)):
            return answer = "No."
        condition said.typed_nothing = (said == result.err(guess_error.empty)):
            return answer = "Type a letter."
        condition said.typed_other = (said == result.err(guess_error.not_one_letter)):
            return answer = "\"{typed}\" is not one letter."
        condition said.typed_again = (said == result.err(guess_error.already_guessed)):
            return answer = "You have already guessed {typed}."


----------------------------------------------------------------------------------------------------------------
Shows how a game ended: the gallows, and whether the player won or was hanged.

game: The game, as game.

console: Where the end is shown, as console.

fails console_error: The console could not be written.
----------------------------------------------------------------------------------------------------------------
function show_end(game, injected console) fails console_error:
    final_picture = gallows(game.miss_count)
    console.write_line(final_picture) or fail

    if won = (outcome_of(game) == outcome.won):
        console.write_line("You win! The word was {game.word}.") or fail
    else hanged:
        console.write_line("Hanged! The word was {game.word}.") or fail
