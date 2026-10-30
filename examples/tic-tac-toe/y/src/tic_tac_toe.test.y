------------------------------------------------------------------------------------------------------------------------
A game with crosses and noughts in the given squares.

crosses: The squares that hold a cross, as list<uint32>.
noughts: The squares that hold a nought, as list<uint32>.
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


-- Crosses move first, then noughts, then crosses again.
test the_players_take_turns
        covers place, other > current.cross, other > current.nought:
    given:
        mutable game = new_game()
    when:
        place(mutable game, 5)
        place(mutable game, 1)
    then:
        a_cross_first = (game.squares.at_number(5) == optional.some(mark.cross))
        then_a_nought = (game.squares.at_number(1) == optional.some(mark.nought))
        then_a_cross_again = (game.to_move == mark.cross)


-- Three of a player's marks in a line win the game, along a diagonal as along a row.
test three_in_a_row_wins
        covers over, winner > line'many > three:
    given:
        game = marked(crosses: [3, 5, 7], noughts: [1, 2], to_move: mark.nought)
    when:
        found = winner(game)
    then:
        -- Expected the diagonal, the last line looked at, to be found.
        crosses_won = (found == optional.some(mark.cross))
        ended = over(game)


-- A full board with no line of three is worth nothing to either player.
test a_full_board_with_no_line_is_a_draw
        covers winner > line'many > not_three'2, winner > line'many > not_three'3, open_squares > number'many > marked,
               value_of > now.drawn:
    given:
        game = marked(crosses: [1, 2, 6, 7, 9], noughts: [3, 4, 5, 8], to_move: mark.nought)
    when:
        value = value_of(game)
    then:
        drawn = (value == 0)
        ended = over(game)


-- The computer, able both to win and to block a line of the player's, wins.
test the_computer_wins_rather_than_blocks
        covers computer_move > square'many > better, computer_move > square'many > no_better'lt, winner > line > three,
               value_of > now.lost, value_of > now.open > square, value_of > now.open > square'many:
    given:
        game = marked(crosses: [1, 2], noughts: [4, 5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        completes_its_row = (chosen == 6)


-- The computer blocks the player's line of two.
test the_computer_blocks_a_line:
    given:
        game = marked(crosses: [1, 2], noughts: [5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        blocks_the_top_row = (chosen == 3)


-- The board shows each mark, and the number of each empty square.
test the_board_shows_marks_and_numbers
        covers shown > first'many > number'many, square_shown > held.empty, square_shown > held.marked:
    given:
        game = marked(crosses: [5], noughts: [1], to_move: mark.cross)
    when:
        picture = shown(game)
    then:
        drawn = (picture == " O | 2 | 3\n---+---+---\n 4 | X | 6\n---+---+---\n 7 | 8 | 9")


-- A player who never lets the computer make a line draws.
test a_player_who_plays_perfectly_draws
        covers computer_move > square'many > no_better'eq, play > playing'many > persons_turn,
               play > playing'many > computers_turn:
    inject:
            console = fake console(input: ["5", "8", "3", "4", "9"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("The board is full. A draw."))


-- A player who leaves the computer a line loses.
test a_player_who_plays_carelessly_loses
        covers announce > won:
    inject:
            console = fake console(input: ["1", "2", "4"])
            forever = fake forever
    when:
        play(new_game()) or fail
    then:
        announced = (console.written().last() == optional.some("O has three in a row, and wins."))


-- A player who types something that is not a move is told why, and asked again.
test a_player_is_told_what_was_wrong
        covers play > playing'many > persons_turn > persons_move'fails, persons_move > line_read > attempted.invalid,
               persons_move > closed, complaint > wrong.no_square, complaint > wrong.taken:
    inject:
            console = fake console(input: ["five", "10", "5", "5"])
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        told_the_form = console.written().contains("Type the number of a square, from 1 to 9.")
        told_no_square = console.written().contains("There is no such square.")
        told_taken = console.written().contains("That square is taken.")
        closed = (ended == result.err(play_error.input_closed))


-- A console that cannot be read ends the game with the console's error.
test a_failing_console_ends_the_game
        covers winner > line'many > not_three'1, open_squares > number'many > empty,
               play > playing > persons_turn > persons_move'fails, persons_move > typed'fails:
    inject:
            console = fake console(input: [], read_fails: true)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_reading = (ended == result.err(play_error.console(error: console_error.fake_read_failure())))


-- There is no square 0.
test square_zero_is_not_on_the_board
        covers read_move > a_number > judged.off_board'1:
    given:
        game = new_game()
    when:
        refused = read_move(game, "0")
    then:
        no_such_square = (refused == result.err(move_error.no_such_square))


-- With one square left, the computer takes it.
test the_computer_takes_the_last_square
        covers computer_move > square > better:
    given:
        game = marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        the_ninth = (chosen == 9)


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


-- A console that cannot say what is wrong with a move ends the turn.
test a_wrong_move_cannot_be_complained_of
        covers persons_move > line_read > attempted.invalid > write_line'2'fails, complaint > wrong.not_a_move:
    given:
        mutable game = new_game()
    inject:
            console = fake console(input: ["x"], failing_after_writes: 2)
            forever = fake forever
    when:
        moved = persons_move(mutable game)
    then:
        failed_writing = (moved == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A console that cannot write the greeting ends play.
test a_console_that_cannot_greet_ends_play
        covers new_game, play > write_line'1'fails:
    inject:
            console = fake console(input: [], failing_after_writes: 0)
            forever = fake forever
    when:
        ended = play(new_game())
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))


-- A game begun on a full board is over before anyone moves.
test a_full_board_is_over_at_once
        covers play > over_at_once, announce > drawn:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8, 9], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


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


-- The player takes the last square, which ends the game.
test the_player_fills_the_last_square
        covers play > playing > persons_turn, persons_move > line_read > attempted.valid:
    inject:
            console = fake console(input: ["9"])
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


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


-- The computer takes the last square, which ends the game.
test the_computer_fills_the_last_square
        covers play > playing > computers_turn:
    inject:
            console = fake console(input: [])
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4, 8], noughts: [2, 5, 6, 7], to_move: mark.nought))
    then:
        drawn = (console.written().last() == optional.some("The board is full. A draw."))


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


-- A console that cannot show the end, after the player's move and the computer's, ends play.
test a_console_that_cannot_announce_after_both_have_moved_ends_play
        covers play > playing'many > persons_turn > announce'fails,
               play > playing'many > computers_turn > announce'fails:
    inject:
            console = fake console(input: ["8"], failing_after_writes: 4)
            forever = fake forever
    when:
        ended = play(marked(crosses: [1, 3, 4], noughts: [2, 5, 6, 7], to_move: mark.cross))
    then:
        failed_writing = (ended == result.err(play_error.console(error: console_error.fake_write_failure())))
