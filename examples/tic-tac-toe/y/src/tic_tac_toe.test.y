------------------------------------------------------------------------------------------------------------------------
A game with crosses and noughts in the given squares.

crosses: The squares that hold a cross, as list<element type=uint32>.
noughts: The squares that hold a nought, as list<element type=uint32>.
to_move: Whose turn it is, as mark.

made: The game, as game.
------------------------------------------------------------------------------------------------------------------------
function marked(crosses, noughts, to_move) returns made:
    mutable squares = list.repeated(optional.none, times: board.squares)

    for square in crosses:
        squares.set_number(square, optional.some(mark.cross))
    else no_crosses

    for square in noughts:
        squares.set_number(square, optional.some(mark.nought))
    else no_noughts
    return made = game(squares: squares, to_move: to_move)


-- A game starts with every square empty, and crosses to move.
test a_game_starts_with_an_empty_board_and_crosses_to_move
        covers new_game:
    when:
        started = new_game()
    then:
        empty = (started.squares == list.repeated(optional.none, times: board.squares))
        crosses_to_move = (started.to_move == mark.cross)


-- A move is read as a square's number, with any spaces around it.
test a_move_is_read
        covers read_move > a_number > judged.empty:
    given:
        game = new_game()
    when:
        chosen = read_move(game, " 5 ") or fail
    then:
        the_middle = (chosen == 5)


-- A word is not a move.
test a_word_is_not_a_move
        covers read_move > not_a_number:
    given:
        game = new_game()
    when:
        refused = read_move(game, "five")
    then:
        refused_as_not_a_number = (refused == result.err(move_error.not_a_number))


-- There is no square 0.
test square_zero_is_not_on_the_board
        covers read_move > a_number > judged.off_board'1:
    given:
        game = new_game()
    when:
        refused = read_move(game, "0")
    then:
        no_such_square = (refused == result.err(move_error.no_such_square))


-- The board has nine squares, and no tenth.
test there_is_no_tenth_square
        covers read_move > a_number > judged.off_board'2:
    given:
        game = new_game()
    when:
        refused = read_move(game, "10")
    then:
        refused_as_no_square = (refused == result.err(move_error.no_such_square))


-- A square that holds a mark cannot be played again.
test a_marked_square_is_taken
        covers read_move > a_number > judged.taken:
    given:
        game = marked(crosses: [5], noughts: [], to_move: mark.nought)
    when:
        refused = read_move(game, "5")
    then:
        refused_as_taken = (refused == result.err(move_error.taken))


-- A move puts the mark of the player to move in the square, and passes the turn.
test a_move_puts_a_mark_and_passes_the_turn
        covers place:
    given:
        mutable game = new_game()
    when:
        place(mutable game, 5)
    then:
        a_cross_in_the_middle = (game.squares.at_number(5) == optional.some(mark.cross))
        noughts_to_move = (game.to_move == mark.nought)


-- With one square left, the computer takes it.
test the_computer_takes_the_last_square
        covers computer_move > square > better:
    given:
        game = marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        the_ninth = (chosen == 9)


-- The computer, able both to win and to block a line of the player's, wins.
test the_computer_wins_rather_than_blocks
        covers computer_move > square'many > better:
    given:
        game = marked(crosses: [1, 2], noughts: [4, 5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        completes_its_row = (chosen == 6)


-- The computer blocks the player's line of two, and every other square is worse.
test the_computer_blocks_a_line
        covers computer_move > square'many > no_better'lt:
    given:
        game = marked(crosses: [1, 2], noughts: [5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        blocks_the_top_row = (chosen == 3)


-- Of squares that are as good as each other, the computer takes the first.
test of_squares_as_good_the_computer_takes_the_first
        covers computer_move > square'many > no_better'eq:
    given:
        game = marked(crosses: [1, 9], noughts: [5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        the_first_of_the_edges = (chosen == 2)


-- A game ends when a player has three in a row.
test a_line_of_three_ends_the_game
        covers over:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    when:
        ended = over(game)
    then:
        won = ended


-- A game ends when the board is full.
test a_full_board_ends_the_game
        covers over:
    given:
        game = marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)
    when:
        ended = over(game)
    then:
        full = ended


-- A game with empty squares and no line of three goes on.
test a_new_game_is_not_over
        covers over:
    given:
        game = new_game()
    when:
        ended = over(game)
    then:
        still_playing = not ended


-- A line along the top row, the first line looked at, wins.
test a_line_along_the_top_row_is_found_first
        covers winner > line > three:
    given:
        game = marked(crosses: [1, 2, 3], noughts: [4, 5], to_move: mark.nought)
    when:
        found = winner(game)
    then:
        crosses_won = (found == optional.some(mark.cross))


-- Three of a player's marks in a line win the game, along a diagonal as along a row.
test three_in_a_row_wins
        covers winner > line'many > three:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    when:
        found = winner(game)
    then:
        -- Expected the diagonal, the last line looked at, to be found.
        crosses_won = (found == optional.some(mark.cross))


-- On an empty board no line has a mark at its start, so no one has won.
test an_empty_board_has_no_winner
        covers winner > line'many > not_three'1:
    given:
        game = new_game()
    when:
        found = winner(game)
    then:
        no_winner = (found == optional.none)


-- A line whose first two squares hold different marks wins for no one.
test a_line_with_two_marks_that_differ_is_not_three
        covers winner > line'many > not_three'2:
    given:
        game = marked(crosses: [1], noughts: [2], to_move: mark.cross)
    when:
        found = winner(game)
    then:
        no_winner = (found == optional.none)


-- Two of a player's marks in a line, with the third square empty, win for no one.
test two_in_a_row_are_not_three
        covers winner > line'many > not_three'3:
    given:
        game = marked(crosses: [1, 2], noughts: [4], to_move: mark.nought)
    when:
        found = winner(game)
    then:
        no_winner = (found == optional.none)


-- After crosses, noughts move.
test crosses_are_followed_by_noughts
        covers other > current.cross:
    when:
        next = other(mark.cross)
    then:
        noughts_next = (next == mark.nought)


-- After noughts, crosses move.
test noughts_are_followed_by_crosses
        covers other > current.nought:
    when:
        next = other(mark.nought)
    then:
        crosses_next = (next == mark.cross)


-- Every square of a new game is open, in the order they are numbered.
test every_square_of_a_new_game_is_open
        covers open_squares > number'many > empty:
    given:
        game = new_game()
    when:
        open = open_squares(game)
    then:
        all_nine = (open == [1, 2, 3, 4, 5, 6, 7, 8, 9])


-- A full board has no open square.
test a_full_board_has_no_open_square
        covers open_squares > number'many > marked:
    given:
        game = marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)
    when:
        open = open_squares(game)
    then:
        none_open = (open.count() == 0)


-- A game the player to move has lost costs one, and one more for each open square: here four.
test a_lost_game_costs_more_the_more_squares_are_left
        covers value_of > now.lost:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    when:
        value = value_of(game)
    then:
        lost_with_four_left = (value == -5)


-- A full board with no line of three is worth nothing to either player.
test a_full_board_with_no_line_is_a_draw
        covers value_of > now.drawn:
    given:
        game = marked(crosses: [1, 2, 6, 7, 9], noughts: [3, 4, 5, 8], to_move: mark.nought)
    when:
        value = value_of(game)
    then:
        drawn = (value == 0)


-- A last square that makes no line leads to a draw, worth nothing.
test a_last_square_that_makes_no_line_is_worth_nothing
        covers value_of > now.open > square:
    given:
        game = marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.cross)
    when:
        value = value_of(game)
    then:
        drawn = (value == 0)


-- A player who can make a line at once, leaving four squares open, wins with a value of five.
test a_line_that_can_be_made_at_once_is_worth_most
        covers value_of > now.open > square'many:
    given:
        game = marked(crosses: [1, 2], noughts: [4, 5], to_move: mark.cross)
    when:
        value = value_of(game)
    then:
        wins_at_once = (value == 5)


-- The board shows each mark, and the number of each empty square.
test the_board_shows_marks_and_numbers
        covers shown > first'many > number'many:
    given:
        game = marked(crosses: [5], noughts: [1], to_move: mark.cross)
    when:
        picture = shown(game)
    then:
        drawn = (picture == " O | 2 | 3\n---+---+---\n 4 | X | 6\n---+---+---\n 7 | 8 | 9")


-- An empty square shows its number, so the player sees what to type.
test an_empty_square_shows_its_number
        covers square_shown > held.empty:
    given:
        game = new_game()
    when:
        drawn = square_shown(game, 5)
    then:
        its_number = (drawn == "5")


-- A marked square shows its mark.
test a_marked_square_shows_its_mark
        covers square_shown > held.marked:
    given:
        game = marked(crosses: [5], noughts: [1], to_move: mark.cross)
    when:
        drawn = square_shown(game, 1)
    then:
        its_mark = (drawn == "O")


-- A game begun on a full board is over before anyone moves.
test a_full_board_is_over_at_once
        covers play > over_at_once:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        play(marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)) or fail
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


-- The player takes the last square, which ends the game.
test the_player_fills_the_last_square
        covers play > playing > persons_turn:
    inject:
            console = fake console(input: ["9"])
            forever = fake forever
    when:
        play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.cross)) or fail
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


-- The computer takes the last square, which ends the game.
test the_computer_fills_the_last_square
        covers play > playing > computers_turn:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought)) or fail
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


-- A player who never lets the computer make a line draws, with the last move.
test a_player_who_plays_perfectly_draws
        covers play > playing'many > persons_turn:
    inject:
            console = fake console(input: ["5", "8", "3", "4", "9"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("The board is full. A draw."))


-- A player who leaves the computer a line loses to the computer's last move.
test a_player_who_plays_carelessly_loses
        covers play > playing'many > computers_turn:
    inject:
            console = fake console(input: ["1", "2", "4"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("O has three in a row, and wins."))


-- A console that cannot write the greeting ends play.
test a_console_that_cannot_greet_ends_play
        covers play > write_line'1'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 0)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
        covers play > playing > persons_turn > persons_move'fails:
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


-- Input that closes after a wrong move, before the player moves, ends play.
test input_that_closes_after_a_wrong_move_ends_play
        covers play > playing'many > persons_turn > persons_move'fails:
    inject:
            console = fake console(input: ["0"])
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot tell the computer's only move ends play.
test a_console_that_cannot_tell_the_computers_last_move_ends_play
        covers play > playing > computers_turn > write_line'2'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot tell the computer's move, after the player's, ends play.
test a_console_that_cannot_tell_the_computers_move_ends_play
        covers play > playing'many > computers_turn > write_line'2'fails:
    inject:
            console = fake console(input: ["8"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end of a game begun on a full board ends play.
test a_console_that_cannot_announce_at_once_ends_play
        covers play > over_at_once > announce'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end after the player's last move ends play.
test a_console_that_cannot_announce_the_players_last_move_ends_play
        covers play > playing > persons_turn > announce'fails:
    inject:
            console = fake console(input: ["9"], failing_after_writes: 3)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end after the computer's only move ends play.
test a_console_that_cannot_announce_the_computers_last_move_ends_play
        covers play > playing > computers_turn > announce'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 2)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end, after the player's wrong move and then their last, ends play.
test a_console_that_cannot_announce_after_a_retyped_last_move_ends_play
        covers play > playing'many > persons_turn > announce'fails:
    inject:
            console = fake console(input: ["0", "9"], failing_after_writes: 6)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot show the end, after the player's move and the computer's, ends play.
test a_console_that_cannot_announce_after_both_have_moved_ends_play
        covers play > playing'many > computers_turn > announce'fails:
    inject:
            console = fake console(input: ["8"], failing_after_writes: 4)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- The player's move is made, and the turn passes.
test a_players_move_is_made
        covers persons_move > line_read > attempted.valid:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["5"])
            forever = fake forever
    when:
        persons_move(mutable game) or fail
    then:
        a_cross_in_the_middle = (game.squares.at_number(5) == optional.some(mark.cross))
        noughts_to_move = (game.to_move == mark.nought)


-- A player who types something that is not a move is told why, and nothing is played.
test a_player_is_told_what_was_wrong
        covers persons_move > line_read > attempted.invalid:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["five"])
            forever = fake forever
    when:
        persons_move(mutable game) or fail
    then:
        told_the_form = (console.written().last() == optional.some("Type the number of a square, from 1 to 9."))
        unchanged = (game == new_game())


-- A console that cannot draw the board cannot ask for a move.
test a_move_cannot_be_asked_for_without_the_board
        covers persons_move > write_line'1'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [], failing_after_writes: 0)
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        failed_writing = (moved == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot ask for a move cannot read one.
test a_move_cannot_be_asked_for
        covers persons_move > write'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [], failing_after_writes: 1)
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        failed_writing = (moved == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot be read fails the move with the console's error.
test a_move_cannot_be_read_from_a_failing_console
        covers persons_move > typed'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        failed_reading = (moved == result.err(play_error.console(error: console_error.fake_read_failure())))


-- A console whose input is closed gives no move.
test a_closed_console_gives_no_move
        covers persons_move > closed:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        closed = (moved == result.err(play_error.input_closed))


-- A console that cannot say what is wrong with a move ends the turn.
test a_wrong_move_cannot_be_complained_of
        covers persons_move > line_read > attempted.invalid > write_line'2'fails:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["x"], failing_after_writes: 2)
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        failed_writing = (moved == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A player who types no number is asked for a square's number.
test a_player_who_types_no_number_is_asked_for_one
        covers complaint > wrong.not_a_move:
    when:
        said = complaint(move_error.not_a_number)
    then:
        asks_for_a_number = (said == "Type the number of a square, from 1 to 9.")


-- A player who names no square is told there is none.
test a_player_who_names_no_square_is_told_so
        covers complaint > wrong.no_square:
    when:
        said = complaint(move_error.no_such_square)
    then:
        says_there_is_none = (said == "There is no such square.")


-- A player who names a taken square is told it is taken.
test a_player_who_names_a_taken_square_is_told_so
        covers complaint > wrong.taken:
    when:
        said = complaint(move_error.taken)
    then:
        says_it_is_taken = (said == "That square is taken.")


-- A full board with no line is announced as a draw.
test a_draw_is_announced
        covers announce > drawn:
    given:
        game = marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)
    inject:
            console = fake console(input: [])
    when:
        announce(game) or fail
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


-- A line of three is announced as a win for its player.
test a_win_is_announced
        covers announce > won:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    inject:
            console = fake console(input: [])
    when:
        announce(game) or fail
    then:
        crosses_won = (console.written().last() == optional.some("X has three in a row, and wins."))


-- A console that cannot draw the board cannot show how a game ended.
test an_end_cannot_be_shown_without_the_board
        covers announce > write_line'1'fails:
    given:
        game = marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)
    inject:
            console = fake console(input: [], failing_after_writes: 0)
    when:
        said = announce(game)
    then:
        failed_writing = (said == result.err(console_error.fake_write_failure()))


-- A console that cannot announce a draw cannot show how a game ended.
test a_draw_cannot_be_announced
        covers announce > drawn > write_line'2'fails:
    given:
        game = marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross)
    inject:
            console = fake console(input: [], failing_after_writes: 1)
    when:
        said = announce(game)
    then:
        failed_writing = (said == result.err(console_error.fake_write_failure()))


-- A console that cannot announce a win cannot show how a game ended.
test a_win_cannot_be_announced
        covers announce > won > write_line'3'fails:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    inject:
            console = fake console(input: [], failing_after_writes: 1)
    when:
        said = announce(game)
    then:
        failed_writing = (said == result.err(console_error.fake_write_failure()))
