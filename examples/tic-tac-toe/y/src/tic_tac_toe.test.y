----------------------------------------------------------------------------------------------------------------------------
A game with crosses and noughts in the given squares.

crosses: The squares that hold a cross, as list<uint32>.
noughts: The squares that hold a nought, as list<uint32>.
to_move: Whose turn it is, as mark.

made: The game, as game.
----------------------------------------------------------------------------------------------------------------------------
function marked(crosses, noughts, to_move) returns made:
    mutable squares = list.repeated(optional.none, times: board.squares)

    for square in crosses:
        squares.set_number(square, optional.some(mark.cross))

    for square in noughts:
        squares.set_number(square, optional.some(mark.nought))
    return made = game(squares: squares, to_move: to_move)


-- A move is read as a square's number, with any spaces around it.
test a_move_is_read
        covers read_move#a_number, read_move#judged.empty:
    given:
        game = new_game()
    when:
        chosen = read_move(game, " 5 ") or fail
    then:
        the_middle = (chosen == 5)


-- A word is not a move.
test a_word_is_not_a_move
        covers read_move#not_a_number, read_move!move_error.not_a_number:
    given:
        game = new_game()
    when:
        refused = read_move(game, "five")
    then:
        refused_as_not_a_number = (refused == result.err(move_error.not_a_number))


-- The board has nine squares, and no tenth.
test there_is_no_tenth_square
        covers read_move#judged.off_board, read_move!move_error.no_such_square:
    given:
        game = new_game()
    when:
        refused = read_move(game, "10")
    then:
        refused_as_no_square = (refused == result.err(move_error.no_such_square))


-- A square that holds a mark cannot be played again.
test a_marked_square_is_taken
        covers read_move#judged.taken, read_move!move_error.taken:
    given:
        game = marked(crosses: [5], noughts: [], to_move: mark.nought)
    when:
        refused = read_move(game, "5")
    then:
        refused_as_taken = (refused == result.err(move_error.taken))


-- Crosses move first, then noughts, then crosses again.
test the_players_take_turns
        covers other#current.cross, other#current.nought:
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
        covers winner#not_three, winner#three:
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
        covers value_of#now.drawn:
    given:
        game = marked(crosses: [1, 2, 6, 7, 9], noughts: [3, 4, 5, 8], to_move: mark.nought)
    when:
        value = value_of(game)
    then:
        drawn = (value == 0)
        ended = over(game)


-- The computer, able both to win and to block a line of the player's, wins.
test the_computer_wins_rather_than_blocks
        covers computer_move#better, computer_move#no_better, value_of#now.lost, value_of#now.open, open_squares#marked,
               open_squares#empty:
    given:
        game = marked(crosses: [1, 2], noughts: [4, 5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        completes_its_row = (chosen == 6)


-- The computer blocks the player's line of two.
test the_computer_blocks_a_line
        covers computer_move#better:
    given:
        game = marked(crosses: [1, 2], noughts: [5], to_move: mark.nought)
    when:
        chosen = computer_move(game)
    then:
        blocks_the_top_row = (chosen == 3)


-- The board shows each mark, and the number of each empty square.
test the_board_shows_marks_and_numbers
        covers square_shown#held.empty, square_shown#held.marked:
    given:
        game = marked(crosses: [5], noughts: [1], to_move: mark.cross)
    when:
        picture = shown(game)
    then:
        drawn = (picture == " O | 2 | 3\n---+---+---\n 4 | X | 6\n---+---+---\n 7 | 8 | 9")


-- A player who never lets the computer make a line draws.
test a_player_who_plays_perfectly_draws
        covers play#playing, play#persons_turn, play#line_read, play#attempted.valid, play#computers_turn, play#game_over,
               play#drawn:
    inject:
            console = fake console(input: ["5", "8", "3", "4", "9"])
            forever = fake forever
    when:
        play() or fail
    then:
        announced = (console.written().last() == optional.some("The board is full. A draw."))


-- A player who leaves the computer a line loses.
test a_player_who_plays_carelessly_loses
        covers play#won:
    inject:
            console = fake console(input: ["1", "2", "4"])
            forever = fake forever
    when:
        play() or fail
    then:
        announced = (console.written().last() == optional.some("O has three in a row, and wins."))


-- A player who types something that is not a move is told why, and asked again.
test a_player_is_told_what_was_wrong
        covers play#attempted.not_a_move, play#attempted.no_square, play#attempted.taken, play#closed,
               play!play_error.input_closed:
    inject:
            console = fake console(input: ["five", "10", "5", "5"])
            forever = fake forever
    when:
        ended = play()
    then:
        told_the_form = console.written().contains("Type the number of a square, from 1 to 9.")
        told_no_square = console.written().contains("There is no such square.")
        told_taken = console.written().contains("That square is taken.")
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
