-- A letter in the word is a hit, and shows wherever it occurs in the word.
test a_letter_in_the_word_is_a_hit
    covers guess#nothing_typed.else, guess#not_one_rune.else, guess#not_a_letter.else, guess#repeated.else,
           guess#in_word, shown_rune#known, shown_rune#known.else
{
    given {
        variable game = new_game(word.new("gallows")?)
    }
    when {
        let verdict = guess(mutable game, "l")?
    }
    then {
        assert hit = (verdict == .hit)
        assert shown_twice = (masked(game) == "_ _ l l _ _ _")
    }
}

-- A letter not in the word is a miss, and is kept in the order it was guessed.
test a_letter_not_in_the_word_is_a_miss
    covers guess#in_word.else
{
    given {
        variable game = new_game(word.new("rope")?)
    }
    when {
        let verdict = guess(mutable game, "z")?
    }
    then {
        assert miss = (verdict == .miss)
        assert recorded = (game.misses == ['z'])
    }
}

-- A letter guessed before is refused in either case, and costs nothing.
test a_repeated_letter_is_refused
    covers guess#repeated, guess!already_guessed
{
    given {
        variable game = game { word: word.new("rope")?, hits: set.new(), misses: ['z'] }
    }
    when {
        let refused = guess(mutable game, "Z")
    }
    then {
        assert refused_as_repeated = (refused == .err(.already_guessed(letter: 'z')))
        assert no_new_miss = (game.misses.count() == 1)
    }
}

-- A line of spaces is not a guess.
test typing_nothing_is_not_a_guess
    covers guess#nothing_typed, guess!empty
{
    given {
        variable game = new_game(word.new("rope")?)
    }
    when {
        let refused = guess(mutable game, "   ")
    }
    then {
        assert refused_as_empty = (refused == .err(.empty))
    }
}

-- Two letters are not a guess.
test two_letters_are_not_a_guess
    covers guess#not_one_rune, guess!not_one_letter
{
    given {
        variable game = new_game(word.new("rope")?)
    }
    when {
        let refused = guess(mutable game, "ab")
    }
    then {
        assert refused_as_two = (refused == .err(.not_one_letter(text: "ab")))
    }
}

-- A digit is not a guess.
test a_digit_is_not_a_guess
    covers guess#not_a_letter
{
    given {
        variable game = new_game(word.new("rope")?)
    }
    when {
        let refused = guess(mutable game, "7")
    }
    then {
        assert refused_as_digit = (refused == .err(.not_one_letter(text: "7")))
    }
}

-- The last miss allowed hangs the man.
test six_misses_hang_the_man
    covers outcome_of#hanged
{
    given {
        let game = game { word: word.new("rope")?, hits: set.new(), misses: ['a', 'b', 'c', 'd', 'f', 'g'] }
    }
    when {
        let ending = outcome_of(game)
    }
    then {
        assert lost = (ending == .lost)
    }
}

-- Finding every letter wins.
test finding_every_letter_wins
    covers outcome_of#hanged.else, outcome_of#solved
{
    given {
        let game = game { word: word.new("rope")?, hits: set.of(['r', 'o', 'p', 'e']), misses: ['a'] }
    }
    when {
        let ending = outcome_of(game)
    }
    then {
        assert won = (ending == .won)
    }
}

-- A new game is still being played.
test a_new_game_is_being_played
    covers outcome_of#solved.else
{
    given {
        let game = new_game(word.new("rope")?)
    }
    when {
        let ending = outcome_of(game)
    }
    then {
        assert playing = (ending == .playing)
    }
}

-- With no misses, the gallows is empty.
test the_empty_gallows
    covers gallows#has_head.else, gallows#has_body.else, gallows#has_left_arm.else, gallows#has_right_arm.else,
           gallows#has_left_leg.else, gallows#has_right_leg.else
{
    when {
        let picture = gallows(0)
    }
    then {
        assert empty = (picture == ["  +---+", "  |   |", "      |", "      |", "      |", "      |", "========="].join(separator: "\n"))
    }
}

-- With every miss allowed, the whole man hangs.
test the_full_gallows
    covers gallows#has_head, gallows#has_body, gallows#has_left_arm, gallows#has_right_arm, gallows#has_left_leg,
           gallows#has_right_leg
{
    when {
        let picture = gallows(6)
    }
    then {
        assert full = (picture == ["  +---+", "  |   |", "  O   |", " /|\\  |", " / \\  |", "      |", "========="].join(separator: "\n"))
    }
}

-- A word list is read word by word, and runs of spaces between words are allowed.
test a_word_list_is_read_in_order
    covers parse_word_list#blank, parse_word_list#blank.else, parse_word_list#empty.else
{
    when {
        let words = parse_word_list("rope  gallows")?
    }
    then {
        assert both_in_order = (words == [word.new("rope")?, word.new("gallows")?])
    }
}

-- A list with no words is refused.
test an_empty_word_list_is_refused
    covers parse_word_list#empty, parse_word_list!empty
{
    when {
        let refused = parse_word_list("   ")
    }
    then {
        assert refused_as_empty = (refused == .err(.empty))
    }
}

-- A list with a short word is refused, and the error names the word.
test a_short_word_is_refused
    covers parse_word_list!not_a_word
{
    when {
        let refused = parse_word_list("rope cat")
    }
    then {
        assert refused_naming_it = (refused == .err(.not_a_word(text: "cat")))
    }
}

-- A player who finds every letter wins, and is told the word.
test a_player_who_finds_every_letter_wins
    covers play#playing, play#playing.else, play#closed.else, play#won
{
    given {
        let words = [word.new("rope")?]
    }
    with {
        console = fake console(input: ["r", "o", "p", "e"])
        random = fake random(seed: 7)
        forever = fake forever
    }
    when {
        play(words)?
    }
    then {
        assert announced = (console.written().last() == .some("You win! The word was rope."))
    }
}

-- A player who misses six times is hanged, and is told the word.
test a_player_who_misses_six_times_is_hanged
    covers play#won.else
{
    given {
        let words = [word.new("rope")?]
    }
    with {
        console = fake console(input: ["a", "b", "c", "d", "f", "g"])
        random = fake random(seed: 7)
        forever = fake forever
    }
    when {
        play(words)?
    }
    then {
        assert announced = (console.written().last() == .some("Hanged! The word was rope."))
    }
}

-- A console that closes in the middle of a game ends it with a failure.
test a_closed_console_ends_the_game
    covers play#closed, play!input_closed
{
    given {
        let words = [word.new("rope")?]
    }
    with {
        console = fake console(input: ["r"])
        random = fake random(seed: 7)
        forever = fake forever
    }
    when {
        let ended = play(words)
    }
    then {
        assert closed = (ended == .err(.input_closed))
    }
}

-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
    covers play!console
{
    given {
        let words = [word.new("rope")?]
    }
    with {
        console = fake console(input: [], read_fails: true)
        random = fake random(seed: 7)
        forever = fake forever
    }
    when {
        let ended = play(words)
    }
    then {
        assert failed_reading = (ended == .err(.console(error: console_error.fake_read_failure())))
    }
}
