-- A letter in the word is a hit, and shows wherever it occurs in the word.
test a_letter_in_the_word_is_a_hit
    covers guess#nothing_typed.else, guess#not_one_rune.else, guess#not_a_letter.else, guess#repeated.else,
           guess#in_word, shown_rune#known, shown_rune#known.else
{
    given {
        variable game = new_game(Word.new("gallows")?)
    }
    when {
        let verdict = guess(&mutable game, "l")?
    }
    then {
        assert hit = (verdict == .Hit)
        assert shown_twice = (masked(&game) == "_ _ l l _ _ _")
    }
}

-- A letter not in the word is a miss, and is kept in the order it was guessed.
test a_letter_not_in_the_word_is_a_miss
    covers guess#in_word.else
{
    given {
        variable game = new_game(Word.new("rope")?)
    }
    when {
        let verdict = guess(&mutable game, "z")?
    }
    then {
        assert miss = (verdict == .Miss)
        assert recorded = (game.misses == ['z'])
    }
}

-- A letter guessed before is refused in either case, and costs nothing.
test a_repeated_letter_is_refused
    covers guess#repeated, guess!AlreadyGuessed
{
    given {
        variable game = Game { word: Word.new("rope")?, hits: Set.new(), misses: ['z'] }
    }
    when {
        let refused = guess(&mutable game, "Z")
    }
    then {
        assert refused_as_repeated = (refused == .Err(.AlreadyGuessed(letter: 'z')))
        assert no_new_miss = (game.misses.count() == 1)
    }
}

-- A line of spaces is not a guess.
test typing_nothing_is_not_a_guess
    covers guess#nothing_typed, guess!Empty
{
    given {
        variable game = new_game(Word.new("rope")?)
    }
    when {
        let refused = guess(&mutable game, "   ")
    }
    then {
        assert refused_as_empty = (refused == .Err(.Empty))
    }
}

-- Two letters are not a guess.
test two_letters_are_not_a_guess
    covers guess#not_one_rune, guess!NotOneLetter
{
    given {
        variable game = new_game(Word.new("rope")?)
    }
    when {
        let refused = guess(&mutable game, "ab")
    }
    then {
        assert refused_as_two = (refused == .Err(.NotOneLetter(text: "ab")))
    }
}

-- A digit is not a guess.
test a_digit_is_not_a_guess
    covers guess#not_a_letter
{
    given {
        variable game = new_game(Word.new("rope")?)
    }
    when {
        let refused = guess(&mutable game, "7")
    }
    then {
        assert refused_as_digit = (refused == .Err(.NotOneLetter(text: "7")))
    }
}

-- The last miss allowed hangs the man.
test six_misses_hang_the_man
    covers outcome#hanged
{
    given {
        let game = Game { word: Word.new("rope")?, hits: Set.new(), misses: ['a', 'b', 'c', 'd', 'f', 'g'] }
    }
    when {
        let ending = outcome(&game)
    }
    then {
        assert lost = (ending == .Lost)
    }
}

-- Finding every letter wins.
test finding_every_letter_wins
    covers outcome#hanged.else, outcome#solved
{
    given {
        let game = Game { word: Word.new("rope")?, hits: Set.of(['r', 'o', 'p', 'e']), misses: ['a'] }
    }
    when {
        let ending = outcome(&game)
    }
    then {
        assert won = (ending == .Won)
    }
}

-- A new game is still being played.
test a_new_game_is_being_played
    covers outcome#solved.else
{
    given {
        let game = new_game(Word.new("rope")?)
    }
    when {
        let ending = outcome(&game)
    }
    then {
        assert playing = (ending == .Playing)
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
        assert both_in_order = (words == [Word.new("rope")?, Word.new("gallows")?])
    }
}

-- A list with no words is refused.
test an_empty_word_list_is_refused
    covers parse_word_list#empty, parse_word_list!Empty
{
    when {
        let refused = parse_word_list("   ")
    }
    then {
        assert refused_as_empty = (refused == .Err(.Empty))
    }
}

-- A list with a short word is refused, and the error names the word.
test a_short_word_is_refused
    covers parse_word_list!NotAWord
{
    when {
        let refused = parse_word_list("rope cat")
    }
    then {
        assert refused_naming_it = (refused == .Err(.NotAWord(text: "cat")))
    }
}

-- A player who finds every letter wins, and is told the word.
test a_player_who_finds_every_letter_wins
    covers play#playing, play#playing.else, play#closed.else, play#won
{
    given {
        let words = [Word.new("rope")?]
    }
    with {
        console = fake Console(input: ["r", "o", "p", "e"])
        random = fake Random(seed: 7)
        forever = fake Forever
    }
    when {
        play(&words)?
    }
    then {
        assert announced = (console.written().last() == .Some("You win! The word was rope."))
    }
}

-- A player who misses six times is hanged, and is told the word.
test a_player_who_misses_six_times_is_hanged
    covers play#won.else
{
    given {
        let words = [Word.new("rope")?]
    }
    with {
        console = fake Console(input: ["a", "b", "c", "d", "f", "g"])
        random = fake Random(seed: 7)
        forever = fake Forever
    }
    when {
        play(&words)?
    }
    then {
        assert announced = (console.written().last() == .Some("Hanged! The word was rope."))
    }
}

-- A console that closes in the middle of a game ends it with a failure.
test a_closed_console_ends_the_game
    covers play#closed, play!InputClosed
{
    given {
        let words = [Word.new("rope")?]
    }
    with {
        console = fake Console(input: ["r"])
        random = fake Random(seed: 7)
        forever = fake Forever
    }
    when {
        let ended = play(&words)
    }
    then {
        assert closed = (ended == .Err(.InputClosed))
    }
}

-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
    covers play!Console
{
    given {
        let words = [Word.new("rope")?]
    }
    with {
        console = fake Console(input: [], read_fails: true)
        random = fake Random(seed: 7)
        forever = fake Forever
    }
    when {
        let ended = play(&words)
    }
    then {
        assert failed_reading = (ended == .Err(.Console(error: ConsoleError.fake_read_failure())))
    }
}
