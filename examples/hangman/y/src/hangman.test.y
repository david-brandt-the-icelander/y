-- A letter in the word is a hit, and shows wherever it occurs in the word.
test a_letter_in_the_word_is_a_hit
        covers guess > judged.in_word, masked > letter'many, shown_rune > known, shown_rune > unknown:
    given:
        mutable game = new_game(word.new("gallows") or fail)
    when:
        answer = guess(mutable game, "l") or fail
    then:
        hit = (answer == verdict.hit)
        shown_twice = (masked(game) == "_ _ l l _ _ _")


-- A letter not in the word is a miss, and is kept in the order it was guessed.
test a_letter_not_in_the_word_is_a_miss
        covers guess > judged.not_in_word:
    given:
        mutable game = new_game(word.new("rope") or fail)
    when:
        answer = guess(mutable game, "z") or fail
    then:
        miss = (answer == verdict.miss)
        recorded = (game.misses == ['z'])


-- A letter guessed before is refused in either case, and costs nothing.
test a_repeated_letter_is_refused
        covers guess > judged.repeated'2:
    given:
        mutable game = game(word: word.new("rope") or fail, hits: set.new(), misses: ['z'])
    when:
        refused = guess(mutable game, "Z")
    then:
        refused_as_repeated = (refused == result.err(guess_error.already_guessed))
        no_new_miss = (game.misses.count() == 1)


-- A line of spaces is not a guess.
test typing_nothing_is_not_a_guess
        covers guess > judged.nothing_typed:
    given:
        mutable game = new_game(word.new("rope") or fail)
    when:
        refused = guess(mutable game, "   ")
    then:
        refused_as_empty = (refused == result.err(guess_error.empty))


-- Two letters are not a guess.
test two_letters_are_not_a_guess
        covers guess > judged.not_one_rune:
    given:
        mutable game = new_game(word.new("rope") or fail)
    when:
        refused = guess(mutable game, "ab")
    then:
        refused_as_two = (refused == result.err(guess_error.not_one_letter))


-- A digit is not a guess.
test a_digit_is_not_a_guess
        covers guess > judged.not_a_letter:
    given:
        mutable game = new_game(word.new("rope") or fail)
    when:
        refused = guess(mutable game, "7")
    then:
        refused_as_digit = (refused == result.err(guess_error.not_one_letter))


-- The last miss allowed hangs the man.
test six_misses_hang_the_man
        covers outcome_of > judged.hanged:
    given:
        game = game(word: word.new("rope") or fail, hits: set.new(), misses: ['a', 'b', 'c', 'd', 'f', 'g'])
    when:
        ending = outcome_of(game)
    then:
        lost = (ending == outcome.lost)


-- Finding every letter wins.
test finding_every_letter_wins
        covers outcome_of > judged.solved:
    given:
        game = game(word: word.new("rope") or fail, hits: set.of(['r', 'o', 'p', 'e']), misses: ['a'])
    when:
        ending = outcome_of(game)
    then:
        won = (ending == outcome.won)


-- A new game is still being played.
test a_new_game_is_being_played
        covers new_game, outcome_of > judged.undecided:
    given:
        game = new_game(word.new("rope") or fail)
    when:
        ending = outcome_of(game)
    then:
        playing = (ending == outcome.playing)


-- With no misses, the gallows is empty.
test the_empty_gallows
        covers gallows, part > not_yet:
    given:
        expected = [
            "  +---+",
            "  |   |",
            "      |",
            "      |",
            "      |",
            "      |",
            "=========",
        ].join(separator: "\n")
    when:
        picture = gallows(0)
    then:
        empty = (picture == expected)


-- With every miss allowed, the whole man hangs.
test the_full_gallows
        covers part > reached:
    given:
        expected = [
            "  +---+",
            "  |   |",
            "  O   |",
            " /|\\  |",
            " / \\  |",
            "      |",
            "=========",
        ].join(separator: "\n")
    when:
        picture = gallows(6)
    then:
        full = (picture == expected)


-- A word list is read word by word, and runs of spaces between words are allowed.
test a_word_list_is_read_in_order
        covers parse_word_list > piece'many > has_text > checked.valid > has_words,
               parse_word_list > piece'many > blank > has_words:
    when:
        words = parse_word_list("rope  gallows") or fail
    then:
        both_in_order = (words == [word.new("rope") or fail, word.new("gallows") or fail])


-- A list with no words is refused.
test an_empty_word_list_is_refused
        covers parse_word_list > piece'many > blank > empty:
    when:
        refused = parse_word_list("   ")
    then:
        refused_as_empty = (refused == result.err(word_list_error.empty))


-- A list with a short word is refused.
test a_short_word_is_refused
        covers parse_word_list > piece'many > has_text > checked.invalid:
    when:
        refused = parse_word_list("rope cat")
    then:
        refused_as_not_a_word = (refused == result.err(word_list_error.not_a_word))


-- A player who finds every letter wins, and is told the word.
test a_player_who_finds_every_letter_wins
        covers play > playing'many > line_read:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r", "o", "p", "e"])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        play(words) or fail
    then:
        announced = (console.written().last() == optional.some("You win! The word was rope."))


-- A player who misses six times is hanged, and is told the word.
test a_player_who_misses_six_times_is_hanged
        covers show_end > hanged:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["a", "b", "c", "d", "f", "g"])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        play(words) or fail
    then:
        announced = (console.written().last() == optional.some("Hanged! The word was rope."))


-- A player who types something other than a new letter is told what was wrong, and the game goes on.
test a_player_is_told_what_was_wrong
        covers guess > judged.repeated'1, reply > said.typed_nothing, reply > said.typed_other,
               reply > said.typed_again:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["", "ab", "r", "r", "o", "p", "e"])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        play(words) or fail
    then:
        -- Expected the player to be told to type a letter.
        told_to_type = console.written().contains("Type a letter.")
        told_not_one = console.written().contains("\"ab\" is not one letter.")
        told_repeated = console.written().contains("You have already guessed r.")
        still_won = (console.written().last() == optional.some("You win! The word was rope."))


-- A console that closes in the middle of a game ends it with a failure.
test a_closed_console_ends_the_game
        covers play > playing'many > closed:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r"])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
        covers play > playing > typed'fails, show_turn:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: [], read_fails: true)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


-- A list of one word gives that word; one blank piece gives none, and one short word is refused.
test a_list_of_one_piece_is_read
        covers parse_word_list > piece > has_text > checked.invalid,
               parse_word_list > piece > has_text > checked.valid > has_words, parse_word_list > piece > blank > empty:
    when:
        one = parse_word_list("rope")
        blank = parse_word_list("")
        short = parse_word_list("cat")
    then:
        the_word = (one == result.ok([word.new("rope") or fail]))
        none = (blank == result.err(word_list_error.empty))
        refused = (short == result.err(word_list_error.not_a_word))


-- A console that cannot write the greeting ends play.
test a_console_that_cannot_greet_ends_play
        covers play > write_line'1'fails:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: [], failing_after_writes: 0)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the first turn ends play.
test a_console_that_cannot_show_the_first_turn_ends_play
        covers play > playing > show_turn'fails:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the second turn ends play: one write for the greeting, four for the first turn
-- and one for the reply.
test a_console_that_cannot_show_a_later_turn_ends_play
        covers play > playing'many > show_turn'fails, reply > said.miss:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["x"], failing_after_writes: 6)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot reply to the first guess ends play.
test a_console_that_cannot_reply_to_the_first_guess_ends_play
        covers play > playing > line_read > write_line'2'fails, reply > said.hit:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r"], failing_after_writes: 5)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot reply to the second guess ends play.
test a_console_that_cannot_reply_to_a_later_guess_ends_play
        covers play > playing'many > line_read > write_line'2'fails:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r", "o"], failing_after_writes: 10)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end of a game won with one guess ends play.
test a_console_that_cannot_show_a_quick_end_ends_play
        covers play > playing > line_read > show_end'fails:
    given:
        words = [word.new("aaaa") or fail]
    inject:
            console = fake console(input: ["a"], failing_after_writes: 6)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end of a game won with four guesses ends play.
test a_console_that_cannot_show_the_end_ends_play
        covers play > playing'many > line_read > show_end'fails:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r", "o", "p", "e"], failing_after_writes: 21)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A word of one letter, repeated, is found with one guess.
test a_word_of_one_letter_is_found_with_one_guess
        covers play > playing > line_read, show_end > won:
    given:
        words = [word.new("aaaa") or fail]
    inject:
            console = fake console(input: ["a"])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        won = (console.written().last() == optional.some("You win! The word was aaaa."))


-- A console closed before the first guess ends play.
test a_console_closed_at_once_ends_play
        covers play > playing > closed:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: [])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot read the second guess ends play.
test a_console_that_cannot_read_a_later_guess_ends_play
        covers play > playing'many > typed'fails:
    given:
        words = [word.new("rope") or fail]
    inject:
            console = fake console(input: ["r"], failing_after_input: true)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play(words)
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


-- A console that fails after no writes cannot show a turn.
test a_turn_cannot_be_shown_after_no_lines
        covers show_turn > write_line'1'fails:
    given:
        game = new_game(word.new("rope") or fail)
    inject:
            console = fake console(input: [], failing_after_writes: 0)
    when:
        shown = show_turn(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that fails after one writes cannot show a turn.
test a_turn_cannot_be_shown_after_one_lines
        covers show_turn > write_line'2'fails:
    given:
        game = new_game(word.new("rope") or fail)
    inject:
            console = fake console(input: [], failing_after_writes: 1)
    when:
        shown = show_turn(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that fails after two writes cannot show a turn.
test a_turn_cannot_be_shown_after_two_lines
        covers show_turn > write_line'3'fails:
    given:
        game = new_game(word.new("rope") or fail)
    inject:
            console = fake console(input: [], failing_after_writes: 2)
    when:
        shown = show_turn(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that fails after three writes cannot show a turn.
test a_turn_cannot_be_shown_after_three_lines
        covers show_turn > write'fails:
    given:
        game = new_game(word.new("rope") or fail)
    inject:
            console = fake console(input: [], failing_after_writes: 3)
    when:
        shown = show_turn(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that cannot write the gallows cannot show an end.
test an_end_cannot_be_shown_without_the_gallows
        covers show_end > write_line'1'fails:
    given:
        mutable game = new_game(word.new("rope") or fail)
        guess(mutable game, "r") or fail
        guess(mutable game, "o") or fail
        guess(mutable game, "p") or fail
        guess(mutable game, "e") or fail
    inject:
            console = fake console(input: [], failing_after_writes: 0)
    when:
        shown = show_end(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that cannot announce a win cannot show the end.
test a_win_cannot_be_announced
        covers show_end > won > write_line'2'fails:
    given:
        mutable game = new_game(word.new("rope") or fail)
        guess(mutable game, "r") or fail
        guess(mutable game, "o") or fail
        guess(mutable game, "p") or fail
        guess(mutable game, "e") or fail
    inject:
            console = fake console(input: [], failing_after_writes: 1)
    when:
        shown = show_end(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))


-- A console that cannot announce a hanging cannot show the end.
test a_hanging_cannot_be_announced
        covers show_end > hanged > write_line'3'fails:
    given:
        mutable game = new_game(word.new("rope") or fail)
        guess(mutable game, "a") or fail
        guess(mutable game, "b") or fail
        guess(mutable game, "c") or fail
        guess(mutable game, "d") or fail
        guess(mutable game, "f") or fail
        guess(mutable game, "g") or fail
    inject:
            console = fake console(input: [], failing_after_writes: 1)
    when:
        shown = show_end(game)
    then:
        failed_writing = (shown == result.err(console_error.fake_write_failure()))
