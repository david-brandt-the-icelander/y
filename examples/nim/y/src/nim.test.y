-- A move is read as a heap's number and a count, with any spaces around them.
test a_move_is_read
        covers parse_move#two, parse_move#numbers, check#heap_exists, check#judged.fits:
    given:
        game = new_game()
    when:
        chosen = read_move(game, " 2  3 ") or fail
    then:
        heap_two = (chosen.heap == 2)
        three_counters = (chosen.take == 3)


-- One number is not a move.
test one_number_is_not_a_move
        covers parse_move#not_two, parse_move!move_error.not_two_numbers, read_move!move_error.not_two_numbers:
    given:
        game = new_game()
    when:
        refused = read_move(game, "3")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- Two words are not a move.
test two_words_are_not_a_move
        covers parse_move#not_numbers:
    given:
        game = new_game()
    when:
        refused = read_move(game, "two three")
    then:
        refused_as_not_a_move = (refused == result.err(move_error.not_two_numbers))


-- A game of three heaps has no fourth.
test there_is_no_fourth_heap
        covers check#no_such_heap, check!move_error.no_such_heap, read_move!move_error.no_such_heap:
    given:
        game = new_game()
    when:
        refused = read_move(game, "4 1")
    then:
        refused_as_no_heap = (refused == result.err(move_error.no_such_heap))


-- A move must take at least one counter.
test taking_nothing_is_not_a_move
        covers check#judged.nothing_taken, check!move_error.nothing_taken, read_move!move_error.nothing_taken:
    given:
        game = new_game()
    when:
        refused = read_move(game, "1 0")
    then:
        refused_as_nothing_taken = (refused == result.err(move_error.nothing_taken))


-- A move cannot take more than its heap holds.
test taking_too_many_is_not_a_move
        covers check#judged.too_many, check!move_error.too_many, read_move!move_error.too_many:
    given:
        game = new_game()
    when:
        refused = read_move(game, "1 4")
    then:
        refused_as_too_many = (refused == result.err(move_error.too_many))


-- The computer finds the move that leaves a nim-sum of zero, even when it is in the last heap.
test the_computer_finds_the_winning_move
        covers computer_move#searching, computer_move#not_this_heap, computer_move#winning:
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
        covers computer_move#losing:
    given:
        game = game(heaps: [1, 4, 5])
    when:
        chosen = computer_move(game)
    then:
        from_the_largest_heap = (chosen.heap == 3)
        takes_one = (chosen.take == 1)


-- A player who always leaves a nim-sum of zero beats the computer.
test a_player_who_plays_perfectly_wins
        covers play#playing, play#persons_turn, play#line_read, play#attempted.valid, play#computers_turn, play#game_over,
               play#person_won:
    inject:
            console = fake console(input: ["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"])
            forever = fake forever
    when:
        play() or fail
    then:
        announced = (console.written().last() == optional.some("You took the last counter. You win!"))


-- A player who takes one counter at a time loses to the computer.
test a_player_who_plays_carelessly_loses
        covers play#computer_won:
    inject:
            console = fake console(input: ["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"])
            forever = fake forever
    when:
        play() or fail
    then:
        announced = (console.written().last() == optional.some("The computer took the last counter. The computer wins."))


-- A player who types something that is not a move is told why, and asked again.
test a_player_is_told_what_was_wrong
        covers play#attempted.not_a_move, play#attempted.no_heap, play#attempted.none_taken, play#attempted.too_many,
               play#closed, play!play_error.input_closed:
    inject:
            console = fake console(input: ["one", "9 1", "1 0", "1 9"])
            forever = fake forever
    when:
        ended = play()
    then:
        -- Expected the player to be shown how a move is typed.
        told_the_form = console.written().contains("Type a heap's number and how many to take, such as 2 3.")
        told_no_heap = console.written().contains("There is no such heap.")
        told_to_take = console.written().contains("Take at least one counter.")
        told_too_many = console.written().contains("That heap does not hold that many.")
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
        covers play!play_error.console:
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        ended = play()
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))
