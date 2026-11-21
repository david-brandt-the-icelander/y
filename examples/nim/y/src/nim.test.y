-- A new game has every heap full.
test a_new_game_has_every_heap_full
        covers new_game:
    when:
        started = new_game()
    then:
        full = (started.heaps == [3, 4, 5])


-- A move is read as a heap's number and a count, with any spaces around them.
test a_move_is_parsed
        covers parse_move > two > numbers:
    when:
        parsed = parse_move(" 2  3 ") or fail
    then:
        heap_two = (parsed.heap == 2)
        three_counters = (parsed.take == 3)


-- One number is not a move.
test one_number_is_not_a_move
        covers parse_move > not_two'lt:
    when:
        refused = parse_move("3")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- Three numbers are not a move.
test three_numbers_are_not_a_move
        covers parse_move > not_two'gt:
    when:
        refused = parse_move("1 2 3")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- A word where the heap's number should be is not a move.
test a_word_for_the_heap_is_not_a_move
        covers parse_move > two > not_numbers'1:
    when:
        refused = parse_move("x 2")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- A word where the count should be is not a move.
test a_word_for_the_count_is_not_a_move
        covers parse_move > two > not_numbers'2:
    when:
        refused = parse_move("2 x")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- Heap 0 does not exist: heaps are numbered from 1.
test there_is_no_heap_zero
        covers check > no_such_heap'1:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 0, take: 1))
    then:
        refused_as_no_heap = (refused == result.err(move_error.no_such_heap))


-- A game of three heaps has no fourth.
test there_is_no_fourth_heap
        covers check > no_such_heap'2:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 4, take: 1))
    then:
        refused_as_no_heap = (refused == result.err(move_error.no_such_heap))


-- A move must take at least one counter.
test taking_nothing_is_not_a_move
        covers check > heap_exists'2lt > judged.nothing_taken:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 1, take: 0))
    then:
        refused_as_nothing_taken = (refused == result.err(move_error.nothing_taken))


-- A move cannot take more than its heap holds.
test taking_too_many_is_not_a_move
        covers check > heap_exists'2lt > judged.too_many:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 1, take: 4))
    then:
        refused_as_too_many = (refused == result.err(move_error.too_many))


-- A move that takes no more than its heap holds is allowed.
test a_move_that_fits_is_allowed
        covers check > heap_exists'2lt > judged.fits:
    given:
        game = new_game()
    when:
        allowed = check(game, move(heap: 2, take: 3)) or fail
    then:
        the_same_move = (allowed == move(heap: 2, take: 3))


-- A move from the last heap must take at least one counter too.
test taking_nothing_from_the_last_heap_is_not_a_move
        covers check > heap_exists'2eq > judged.nothing_taken:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 3, take: 0))
    then:
        refused_as_nothing_taken = (refused == result.err(move_error.nothing_taken))


-- A move cannot take more than the last heap holds.
test taking_too_many_from_the_last_heap_is_not_a_move
        covers check > heap_exists'2eq > judged.too_many:
    given:
        game = new_game()
    when:
        refused = check(game, move(heap: 3, take: 6))
    then:
        refused_as_too_many = (refused == result.err(move_error.too_many))


-- A move that takes the whole last heap is allowed.
test taking_the_whole_last_heap_is_allowed
        covers check > heap_exists'2eq > judged.fits:
    given:
        game = new_game()
    when:
        allowed = check(game, move(heap: 3, take: 5)) or fail
    then:
        the_same_move = (allowed == move(heap: 3, take: 5))


-- A line that names a heap and a count the game allows is read as that move.
test a_move_is_read
        covers read_move:
    given:
        game = new_game()
    when:
        chosen = read_move(game, " 2  3 ") or fail
    then:
        heap_two = (chosen.heap == 2)
        three_counters = (chosen.take == 3)


-- A line that is not two numbers is refused before the game is looked at.
test a_line_that_is_not_a_move_is_refused
        covers read_move > parsed'fails:
    given:
        game = new_game()
    when:
        refused = read_move(game, "two three")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- Two numbers the game does not allow are refused with the reason the check gives.
test a_move_the_game_does_not_allow_is_refused
        covers read_move > chosen'fails:
    given:
        game = new_game()
    when:
        refused = read_move(game, "4 1")
    then:
        refused_as_no_heap = (refused == result.err(move_error.no_such_heap))


-- Making a move takes its counters from its heap.
test a_move_takes_counters_from_its_heap
        covers remove:
    given:
        mutable game = new_game()
    when:
        remove(mutable game, move(heap: 2, take: 3))
    then:
        one_left_in_heap_two = (game.heaps == [3, 1, 5])


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


-- The computer takes a lone heap whole, which wins.
test the_computer_takes_a_lone_heap
        covers computer_move > winnable > number > winning:
    given:
        game = game(heaps: [3])
    when:
        chosen = computer_move(game)
    then:
        all_of_it = (chosen == move(heap: 1, take: 3))


-- The computer finds the move that leaves a nim-sum of zero, even when it is in the last heap.
test the_computer_finds_the_winning_move
        covers computer_move > winnable > number'many > winning:
    given:
        game = game(heaps: [1, 2, 4])
    when:
        chosen = computer_move(game)
    then:
        from_the_last_heap = (chosen.heap == 3)
        leaves_zero = (nim_sum([1, 2, 3]) == 0)
        takes_one = (chosen.take == 1)


-- A game of no heaps has no counters left.
test no_heaps_leave_no_counters
        covers counters_left > no_heaps:
    given:
        none = game(heaps: [])
    when:
        left = counters_left(none)
    then:
        nothing_left = (left == 0)


-- A lone heap's counters are all that is left.
test a_lone_heap_is_all_that_is_left
        covers counters_left > heap:
    given:
        one = game(heaps: [5])
    when:
        left = counters_left(one)
    then:
        five_left = (left == 5)


-- The counters left are those of every heap together.
test the_counters_of_every_heap_are_counted
        covers counters_left > heap'many:
    given:
        game = new_game()
    when:
        left = counters_left(game)
    then:
        twelve_left = (left == 12)


-- The nim-sum of no heaps is zero.
test no_heaps_sum_to_zero
        covers nim_sum > no_heaps:
    when:
        sum = nim_sum([])
    then:
        sum_zero = (sum == 0)


-- The nim-sum of a lone heap is its size.
test a_lone_heap_sums_to_its_size
        covers nim_sum > heap:
    when:
        sum = nim_sum([5])
    then:
        sum_five = (sum == 5)


-- The nim-sum of several heaps combines their sizes with exclusive or.
test heaps_are_summed_with_exclusive_or
        covers nim_sum > heap'many:
    when:
        sum = nim_sum([3, 4, 5])
    then:
        sum_two = (sum == 2)


-- A game of no heaps draws nothing.
test no_heaps_draw_nothing
        covers shown > no_heaps:
    given:
        none = game(heaps: [])
    when:
        picture = shown(none)
    then:
        nothing_shown = (picture == "")


-- A lone heap is drawn as one numbered line.
test a_lone_heap_is_drawn_on_one_line
        covers shown > heap:
    given:
        one = game(heaps: [5])
    when:
        picture = shown(one)
    then:
        one_line = (picture == "  1: |||||")


-- Several heaps are drawn one line each, numbered from 1.
test heaps_are_drawn_one_line_each
        covers shown > heap'many:
    given:
        game = game(heaps: [1, 2])
    when:
        picture = shown(game)
    then:
        two_lines = (picture == "  1: |\n  2: ||")


-- A player who always leaves a nim-sum of zero beats the computer.
test a_player_who_plays_perfectly_wins
        covers play > playing'many > persons_turn:
    inject:
            console = fake console(input: ["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("You took the last counter. You win!"))


-- A player who takes one counter at a time loses to the computer.
test a_player_who_plays_carelessly_loses
        covers play > playing'many > computers_turn:
    inject:
            console = fake console(input: ["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- A player who types something that is not a move is told why, and asked again until the console closes.
test a_player_is_told_what_was_wrong:
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
test a_failing_console_ends_the_game:
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


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
        covers play > playing > write_line'2'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot draw the heaps a second time ends play.
test a_console_that_cannot_draw_the_heaps_again_ends_play
        covers play > playing'many > write_line'2'fails:
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot ask for a move ends play.
test a_console_that_cannot_ask_for_a_move_ends_play
        covers play > playing > persons_turn > turn'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 2)
            forever = fake forever
    when:
        ended = play(game(heaps: [3, 4, 5]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that fails on the player's second turn ends play.
test a_console_that_fails_on_a_later_turn_ends_play
        covers play > playing'many > persons_turn > turn'fails:
    inject:
            console = fake console(input: ["1 1", "2 1"], failing_after_writes: 7)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1, 1]))
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
        covers play > playing'many > computers_turn > announce'fails:
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 5)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce the player's win after several moves ends play.
test a_console_that_cannot_announce_a_win_ends_play
        covers play > playing'many > persons_turn > announce'fails:
    inject:
            console = fake console(input: ["1 1", "3 1"], failing_after_writes: 7)
            forever = fake forever
    when:
        ended = play(game(heaps: [1, 1, 1]))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot announce a win made with one move ends play.
test a_console_that_cannot_announce_a_quick_win_ends_play
        covers play > playing > persons_turn > announce'fails:
    inject:
            console = fake console(input: ["1 2"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(game(heaps: [2]))
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
        covers play > nothing_to_play:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        ended = play(game(heaps: [0]))
    then:
        played = (ended == result.ok)
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- A player who takes a lone heap whole wins with one move.
test one_move_can_win_a_game
        covers play > playing > persons_turn:
    inject:
            console = fake console(input: ["1 2"])
            forever = fake forever
    when:
        ended = play(game(heaps: [2]))
    then:
        played = (ended == result.ok)
        announced = (console.written().last() == optional.some("You took the last counter. You win!"))


-- A console that cannot ask for a move fails the player's move.
test a_move_cannot_be_asked_for
        covers persons_move > write'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["1 1"], failing_after_writes: 0)
            forever = fake forever
    when:
        next = persons_move(mutable game)
    then:
        failed_writing = (next == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot be read fails the player's move.
test a_move_cannot_be_read
        covers persons_move > typed'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        next = persons_move(mutable game)
    then:
        failed_reading = (next == result.err(play_error.console(error: console_error.fake_read_failure())))


-- A console that closes before the player moves fails the move.
test a_closed_console_has_no_move
        covers persons_move > closed:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        next = persons_move(mutable game)
    then:
        closed = (next == result.err(play_error.input_closed))


-- A move the game allows is made, and the computer moves next.
test a_players_move_is_made
        covers persons_move > line_read > attempted.valid:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["2 3"])
            forever = fake forever
    when:
        next = persons_move(mutable game) or fail
    then:
        the_computers_turn = (next == player.computer)
        one_left_in_heap_two = (game.heaps == [3, 1, 5])


-- A line that is not a move is complained about, nothing is taken, and the player moves again.
test a_player_is_told_what_is_wrong_and_asked_again
        covers persons_move > line_read > attempted.invalid:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["x"])
            forever = fake forever
    when:
        next = persons_move(mutable game) or fail
    then:
        the_players_turn_again = (next == player.person)
        told_the_form = console.written().contains("Type a heap's number and how many to take, such as 2 3.")
        nothing_taken = (game.heaps == [3, 4, 5])


-- A console that cannot say what is wrong with a move fails the player's move.
test a_complaint_cannot_be_written
        covers persons_move > line_read > attempted.invalid > write_line'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["x"], failing_after_writes: 1)
            forever = fake forever
    when:
        next = persons_move(mutable game)
    then:
        failed_writing = (next == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A line that is not two numbers is answered with how a move is typed.
test not_a_move_is_answered_with_the_form
        covers complaint > wrong.not_a_move:
    when:
        said = complaint(move_error.not_two_numbers)
    then:
        the_form = (said == "Type a heap's number and how many to take, such as 2 3.")


-- A heap that does not exist is answered as such.
test no_such_heap_is_answered
        covers complaint > wrong.no_heap:
    when:
        said = complaint(move_error.no_such_heap)
    then:
        no_heap = (said == "There is no such heap.")


-- A move that takes nothing is answered with a request to take at least one.
test nothing_taken_is_answered
        covers complaint > wrong.none_taken:
    when:
        said = complaint(move_error.nothing_taken)
    then:
        take_one = (said == "Take at least one counter.")


-- A move that takes too many is answered as such.
test too_many_is_answered
        covers complaint > wrong.too_many:
    when:
        said = complaint(move_error.too_many)
    then:
        not_that_many = (said == "That heap does not hold that many.")


-- When it is the player's turn once the last counter is taken, the computer has won.
test the_computers_win_is_announced
        covers announce > computer_won:
    inject:
            console = fake console(input: [])
    when:
        announce(player.person) or fail
    then:
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- When it is the computer's turn once the last counter is taken, the player has won.
test the_players_win_is_announced
        covers announce > person_won:
    inject:
            console = fake console(input: [])
    when:
        announce(player.computer) or fail
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
