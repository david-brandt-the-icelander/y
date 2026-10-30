-- A move is read as a heap's number and a count, with any spaces around them.
test a_move_is_read
        covers parse_move > two > numbers, check > heap_exists'2lt > judged.fits, read_move:
    given:
        game = new_game()
    when:
        chosen = read_move(game, " 2  3 ") or fail
    then:
        heap_two = (chosen.heap == 2)
        three_counters = (chosen.take == 3)


-- One number is not a move.
test one_number_is_not_a_move
        covers parse_move > not_two'lt, read_move > parsed'fails:
    given:
        game = new_game()
    when:
        refused = read_move(game, "3")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- Two words are not a move.
test two_words_are_not_a_move
        covers parse_move > two > not_numbers'1:
    given:
        game = new_game()
    when:
        refused = read_move(game, "two three")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- A game of three heaps has no fourth.
test there_is_no_fourth_heap
        covers check > no_such_heap'2:
    given:
        game = new_game()
    when:
        refused = read_move(game, "4 1")
    then:
        refused_as_no_heap = (refused == result.err(move_error.no_such_heap))


-- A move must take at least one counter.
test taking_nothing_is_not_a_move
        covers check > heap_exists'2lt > judged.nothing_taken:
    given:
        game = new_game()
    when:
        refused = read_move(game, "1 0")
    then:
        refused_as_nothing_taken = (refused == result.err(move_error.nothing_taken))


-- A move cannot take more than its heap holds.
test taking_too_many_is_not_a_move
        covers check > heap_exists'2lt > judged.too_many:
    given:
        game = new_game()
    when:
        refused = read_move(game, "1 4")
    then:
        refused_as_too_many = (refused == result.err(move_error.too_many))


-- The computer finds the move that leaves a nim-sum of zero, even when it is in the last heap.
test the_computer_finds_the_winning_move
        covers computer_move > winnable > number'many > winning, counters_left > heap'many, nim_sum > heap'many:
    given:
        game = game(heaps: [1, 2, 4])
    when:
        chosen = computer_move(game)
    then:
        from_the_last_heap = (chosen.heap == 3)
        leaves_zero = (nim_sum([1, 2, 3]) == 0)
        takes_one = (chosen.take == 1)


-- With no winning move, the computer takes one counter from the largest heap.
test a_losing_computer_takes_one_from_the_largest_heap
        covers computer_move > lost:
    given:
        game = game(heaps: [1, 4, 5])
    when:
        chosen = computer_move(game)
    then:
        from_the_largest_heap = (chosen.heap == 3)
        takes_one = (chosen.take == 1)


-- A player who always leaves a nim-sum of zero beats the computer.
test a_player_who_plays_perfectly_wins
        covers play > playing'many > persons_turn, play > playing'many > computers_turn:
    inject:
            console = fake console(input: ["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("You took the last counter. You win!"))


-- A player who takes one counter at a time loses to the computer.
test a_player_who_plays_carelessly_loses:
    inject:
            console = fake console(input: ["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- A player who types something that is not a move is told why, and asked again.
test a_player_is_told_what_was_wrong
        covers persons_move > closed, persons_move > line_read > attempted.invalid, complaint > wrong.no_heap,
               complaint > wrong.none_taken:
    inject:
            console = fake console(input: ["one", "9 1", "1 0", "1 9"])
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        -- Expected the player to be shown how a move is typed.
        told_the_form = console.written().contains("Type a heap's number and how many to take, such as 2 3.")
        told_no_heap = console.written().contains("There is no such heap.")
        told_to_take = console.written().contains("Take at least one counter.")
        told_too_many = console.written().contains("That heap does not hold that many.")
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
        covers new_game, persons_move > typed'fails:
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


-- Three numbers are not a move, and neither is a number with a word on either side of it.
test odd_lines_are_not_moves
        covers parse_move > not_two'gt, parse_move > two > not_numbers'2:
    given:
        game = new_game()
    when:
        three = read_move(game, "1 2 3")
        word_first = read_move(game, "x 2")
        word_second = read_move(game, "2 x")
    then:
        not_three = (three == result.err(move_error.not_two_numbers))
        not_word_first = (word_first == result.err(move_error.not_two_numbers))
        not_word_second = (word_second == result.err(move_error.not_two_numbers))


-- Heap 0 does not exist; the last heap does, and its moves are checked like any other's.
test the_last_heap_and_heap_zero
        covers check > no_such_heap'1, check > heap_exists'2eq > judged.nothing_taken,
               check > heap_exists'2eq > judged.too_many:
    given:
        game = new_game()
    when:
        zero = read_move(game, "0 1")
        nothing = read_move(game, "3 0")
        too_many = read_move(game, "3 6")
    then:
        no_heap_zero = (zero == result.err(move_error.no_such_heap))
        nothing_taken = (nothing == result.err(move_error.nothing_taken))
        more_than_held = (too_many == result.err(move_error.too_many))


-- One heap is counted, summed and drawn on its own; a game of no heaps has nothing to count, sum or draw.
test one_heap_and_none
        covers counters_left > no_heaps, counters_left > heap, nim_sum > no_heaps, nim_sum > heap, shown > no_heaps,
               shown > heap:
    given:
        one = game(heaps: [5])
        none = game(heaps: [])
    when:
        one_left = counters_left(one)
        one_sum = nim_sum(one.heaps)
        one_shown = shown(one)
        none_left = counters_left(none)
        none_sum = nim_sum(none.heaps)
        none_shown = shown(none)
    then:
        five_left = (one_left == 5)
        sum_five = (one_sum == 5)
        one_line = (one_shown == "  1: |||||")
        nothing_left = (none_left == 0)
        sum_zero = (none_sum == 0)
        nothing_shown = (none_shown == "")


-- The computer takes a lone heap whole, which wins.
test the_computer_takes_a_lone_heap
        covers computer_move > winnable > number > winning:
    given:
        game = game(heaps: [3])
    when:
        chosen = computer_move(game)
    then:
        all_of_it = (chosen == move(heap: 1, take: 3))


-- A console that cannot write the greeting ends play.
test a_console_that_cannot_greet_ends_play
        covers play > write_line'1'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 0)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot draw the heaps ends play.
test a_console_that_cannot_draw_the_heaps_ends_play
        covers shown > heap'many, play > playing > write_line'2'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot draw the heaps a second time ends play.
test a_console_that_cannot_draw_the_heaps_again_ends_play
        covers remove, play > playing'many > write_line'2'fails, persons_move > line_read > attempted.valid:
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot ask for a move ends play.
test a_console_that_cannot_ask_for_a_move_ends_play
        covers play > playing > persons_turn > turn'fails, persons_move > write'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 2)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot say what is wrong with a move ends play.
test a_console_that_cannot_complain_ends_play
        covers persons_move > line_read > attempted.invalid > write_line'fails, complaint > wrong.not_a_move:
    inject:
            console = fake console(input: ["x"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot tell the computer's move ends play.
test a_console_that_cannot_tell_the_computers_move_ends_play
        covers play > playing'many > computers_turn > write_line'3'fails:
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 4)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce the computer's win ends play.
test a_console_that_cannot_announce_the_computers_win_ends_play
        covers play > playing'many > persons_turn > announce'fails,
               play > playing'many > computers_turn > announce'fails:
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 5)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce a win made with one move ends play.
test a_console_that_cannot_announce_a_quick_win_ends_play
        covers check > heap_exists'2eq > judged.fits, play > playing > persons_turn > announce'fails:
    inject:
            console = fake console(input: ["1 2"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(game(heaps: [2]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce the player's win after several moves ends play.
test a_console_that_cannot_announce_a_win_ends_play
        covers play > playing'many > persons_turn > turn'fails, complaint > wrong.too_many:
    inject:
            console = fake console(input: ["1 1", "2 1"], failing_after_writes: 7)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1, 1]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce the end of a game with no counters ends play.
test a_console_that_cannot_announce_at_once_ends_play
        covers play > nothing_to_play > announce'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        ended = play(game(heaps: [0]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A game with no counters is over before anyone moves.
test a_game_with_no_counters_is_over_at_once
        covers play > nothing_to_play, announce > computer_won:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        ended = play(game(heaps: [0]))
    then:
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- A player who takes a lone heap whole wins with one move.
test one_move_can_win_a_game
        covers play > playing > persons_turn, announce > person_won:
    inject:
            console = fake console(input: ["1 2"])
            forever = fake forever
    when:
        ended = play(game(heaps: [2]))
    then:
        announced = (console.written().last() == optional.some("You took the last counter. You win!"))


-- A console that cannot write cannot announce the computer's win.
test a_computers_win_cannot_be_announced
        covers announce > computer_won > write_line'1'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 0)
    when:
        said = announce(player.person)
    then:
        failed_writing = (said == result.err(console_error.fake_write_failure()))


-- A console that cannot write cannot announce the player's win.
test a_players_win_cannot_be_announced
        covers announce > person_won > write_line'2'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 0)
    when:
        said = announce(player.computer)
    then:
        failed_writing = (said == result.err(console_error.fake_write_failure()))
