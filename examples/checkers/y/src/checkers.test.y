--------------------------------------------------------------------------------------------------------------------------------
A game with only the given pieces on the board, at the Club level.

pieces: The pieces and their squares, as list<element=placed>.
to_move: Whose turn it is, as side.

made: The game, as game.
--------------------------------------------------------------------------------------------------------------------------------
function arranged(pieces, to_move) returns made:
    mutable squares = list.repeated(optional.none, times: 64)

    for placed in pieces:
        squares.set_number(number_of(placed.square), optional.some(placed.piece))
    else no_pieces
    return made = game(squares: squares, to_move: to_move, level: levels.all[2], turns: 0, quiet: 0, last: optional.none)


--------------------------------------------------------------------------------------------------------------------------------
A man of a player's on a square.

owner: The player, as side.
row: The square's row, as int32.
column: The square's column, as int32.

made: The man and his square, as placed.
--------------------------------------------------------------------------------------------------------------------------------
function man(owner, row, column) returns made:
    return made = placed(square: square(row: row, column: column), piece: piece(owner: owner, king: false))


--------------------------------------------------------------------------------------------------------------------------------
A king of a player's on a square.

owner: The player, as side.
row: The square's row, as int32.
column: The square's column, as int32.

made: The king and his square, as placed.
--------------------------------------------------------------------------------------------------------------------------------
function king(owner, row, column) returns made:
    return made = placed(square: square(row: row, column: column), piece: piece(owner: owner, king: true))


--------------------------------------------------------------------------------------------------------------------------------
A frame of a fake window in which the player clicks the centre of a square.

row: The square's row, as int32.
column: The square's column, as int32.

made: The frame, as frame.
--------------------------------------------------------------------------------------------------------------------------------
function clicking(row, column) returns made:
    return made = frame(clicks: [centre_of(square(row: row, column: column))])


--------------------------------------------------------------------------------------------------------------------------------
A level that looks at a given number of positions a move, for the tests of the search.

checks: How many positions, as uint32.

made: The level, as level.
--------------------------------------------------------------------------------------------------------------------------------
function trial(checks) returns made:
    return made = level(name: "Trial", checks: checks, weight: 1)


-- A new game has twelve men for each player, on the dark squares of the three rows nearest them, and red to move.
test a_new_game_has_twelve_men_each
        covers new_game > row'many > column'many:
    when:
        started = new_game(levels.all[0])
    then:
        twenty_four = (placed_pieces(started).count() == 24)
        a_red_man = (piece_at(started, square(row: 5, column: 0)) == optional.some(piece(owner: side.red, king: false)))
        a_white_man = (piece_at(started, square(row: 2, column: 1)) == optional.some(piece(owner: side.white, king: false)))
        red_to_move = (started.to_move == side.red)
        no_turns_yet = (started.turns == 0)
        at_the_level = (started.level == levels.all[0])


-- A light square holds no piece when a game starts.
test a_light_square_starts_empty
        covers starting_piece > placed.light:
    when:
        held = starting_piece(square(row: 0, column: 0))
    then:
        empty = (held == optional.none)


-- A dark square of the three rows at the top starts with a white man.
test the_top_rows_start_with_white_men
        covers starting_piece > placed.white_men:
    when:
        held = starting_piece(square(row: 2, column: 1))
    then:
        a_man = (held == optional.some(piece(owner: side.white, king: false)))


-- A dark square of the first of the three rows at the bottom starts with a red man.
test the_first_of_the_bottom_rows_starts_with_a_red_man
        covers starting_piece > placed.red_men'eq:
    when:
        held = starting_piece(square(row: 5, column: 0))
    then:
        a_man = (held == optional.some(piece(owner: side.red, king: false)))


-- A dark square of the bottom row starts with a red man.
test the_bottom_row_starts_with_a_red_man
        covers starting_piece > placed.red_men'gt:
    when:
        held = starting_piece(square(row: 7, column: 0))
    then:
        a_man = (held == optional.some(piece(owner: side.red, king: false)))


-- A dark square of the two rows in the middle starts empty.
test the_middle_rows_start_empty
        covers starting_piece > placed.between:
    when:
        held = starting_piece(square(row: 3, column: 0))
    then:
        empty = (held == optional.none)


-- A player who can capture must, so a man's step is not among the moves.
test a_capture_must_be_taken
        covers legal_moves > must_jump:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 3), man(side.red, 5, 6)], to_move: side.red)
    when:
        moves = legal_moves(game)
    then:
        only_the_jump = (moves.count() == 1)
        jumped_over = (moves[0].captures == [square(row: 4, column: 3)])


-- With nothing to capture, the moves are steps: red has seven at the start.
test the_first_moves_are_steps
        covers legal_moves > no_jump:
    given:
        game = new_game(levels.all[0])
    when:
        moves = legal_moves(game)
    then:
        seven = (moves.count() == 7)
        red_goes_up = (moves[0].path.last().row == moves[0].from.row - 1)
        no_captures = moves[0].captures.is_empty()


-- A step moves the piece, counts a quiet turn, and passes the turn to the other player.
test a_step_is_a_quiet_turn
        covers apply > quiet:
    given:
        mutable game = new_game(levels.all[0])
        step = move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 0), path: [square(row: 4, column: 1)], captures: [])
    when:
        apply(mutable game, step)
    then:
        left = (piece_at(game, square(row: 5, column: 0)) == optional.none)
        arrived = (piece_at(game, square(row: 4, column: 1)) == optional.some(piece(owner: side.red, king: false)))
        one_quiet_turn = (game.quiet == 1)
        one_turn = (game.turns == 1)
        white_to_move = (game.to_move == side.white)
        remembered = (game.last != optional.none)


-- A white man that steps onto the bottom row is crowned.
test a_white_man_stepping_onto_the_bottom_row_is_crowned
        covers apply > quiet:
    given:
        mutable game = arranged([man(side.white, 6, 1), man(side.red, 0, 7)], to_move: side.white)
        step = move(piece: piece(owner: side.white, king: false), from: square(row: 6, column: 1), path: [square(row: 7, column: 0)], captures: [])
    when:
        apply(mutable game, step)
    then:
        crowned = (piece_at(game, square(row: 7, column: 0)) == optional.some(piece(owner: side.white, king: true)))
        red_to_move = (game.to_move == side.red)


-- A jump takes the piece jumped over off the board, and ends the quiet turns.
test a_jump_takes_the_piece_jumped_over
        covers apply > captured:
    given:
        mutable game = arranged([man(side.red, 5, 2), man(side.white, 4, 3)], to_move: side.red)
        game.quiet := 7
        jump = move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 2), path: [square(row: 3, column: 4)], captures: [square(row: 4, column: 3)])
    when:
        apply(mutable game, jump)
    then:
        taken = (piece_at(game, square(row: 4, column: 3)) == optional.none)
        landed = (piece_at(game, square(row: 3, column: 4)) == optional.some(piece(owner: side.red, king: false)))
        no_longer_quiet = (game.quiet == 0)


-- A red man that jumps onto the top row is crowned.
test a_man_crowned_by_a_jump
        covers apply > captured:
    given:
        mutable game = arranged([man(side.red, 2, 1), man(side.white, 1, 2)], to_move: side.red)
        jump = move(piece: piece(owner: side.red, king: false), from: square(row: 2, column: 1), path: [square(row: 0, column: 3)], captures: [square(row: 1, column: 2)])
    when:
        apply(mutable game, jump)
    then:
        crowned = (piece_at(game, square(row: 0, column: 3)) == optional.some(piece(owner: side.red, king: true)))


-- A chain of jumps takes every piece it jumps over.
test a_chain_of_jumps_takes_every_piece_jumped
        covers apply > captured'many:
    given:
        mutable game = arranged([man(side.red, 6, 1), man(side.white, 5, 2), man(side.white, 3, 4)], to_move: side.red)
        chain = move(piece: piece(owner: side.red, king: false), from: square(row: 6, column: 1), path: [square(row: 4, column: 3), square(row: 2, column: 5)], captures: [square(row: 5, column: 2), square(row: 3, column: 4)])
    when:
        apply(mutable game, chain)
    then:
        first_taken = (piece_at(game, square(row: 5, column: 2)) == optional.none)
        second_taken = (piece_at(game, square(row: 3, column: 4)) == optional.none)
        landed = (piece_at(game, square(row: 2, column: 5)) == optional.some(piece(owner: side.red, king: false)))
        one_turn = (game.turns == 1)


-- The person wins when it is the computer's turn and it cannot move.
test the_person_wins_when_the_computer_cannot_move
        covers standing > judged.won:
    given:
        game = arranged([man(side.red, 4, 3)], to_move: side.white)
    when:
        reached = standing(game)
    then:
        won = (reached == outcome.won)


-- The person loses when it is their turn and they cannot move.
test the_person_loses_when_they_cannot_move
        covers standing > judged.lost:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        reached = standing(game)
    then:
        lost = (reached == outcome.lost)


-- As many turns without a capture as the limit draw the game.
test quiet_turns_draw_the_game
        covers standing > judged.drawn'eq:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
    when:
        reached = standing(game)
    then:
        drawn = (reached == outcome.drawn)


-- More turns without a capture than the limit draw the game too.
test more_quiet_turns_still_draw_the_game
        covers standing > judged.drawn'gt:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit + 1
    when:
        reached = standing(game)
    then:
        drawn = (reached == outcome.drawn)


-- A new game is being played.
test a_new_game_is_being_played
        covers standing > judged.playing:
    given:
        game = new_game(levels.all[0])
    when:
        reached = standing(game)
    then:
        playing = (reached == outcome.playing)


-- A win scores the win points less some for each of the person's moves, times the level's weight.
test a_win_scores_by_level_and_moves
        covers score > won:
    given:
        mutable game = arranged([man(side.red, 4, 3)], to_move: side.white)
        game.turns := 59
    when:
        points = score(game)
    then:
        -- Expected 1000 points, less 300 for 30 moves, times 3 at the Club level.
        scored = (points == 2100)


-- A win that took many moves scores the least win, times the level's weight.
test a_slow_win_scores_the_least_win
        covers score > won:
    given:
        mutable game = arranged([man(side.red, 4, 3)], to_move: side.white)
        game.turns := 299
    when:
        points = score(game)
    then:
        -- Expected the least win, 100 points, times 3.
        least = (points == 300)


-- A game the person has not won scores nothing.
test a_loss_scores_nothing
        covers score > unfinished:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        points = score(game)
    then:
        nothing = (points == 0)


-- Red moves first, so after 59 turns red has made 30 moves.
test red_has_made_every_odd_turn
        covers player_moves:
    given:
        mutable game = new_game(levels.all[0])
        game.turns := 59
    when:
        made = player_moves(game)
    then:
        thirty = (made == 30)


-- White moves after red.
test white_moves_after_red
        covers other > current.red:
    when:
        next = other(side.red)
    then:
        white = (next == side.white)


-- Red moves after white.
test red_moves_after_white
        covers other > current.white:
    when:
        next = other(side.white)
    then:
        red = (next == side.red)


-- A player with no pieces on the board has no steps.
test a_player_with_no_pieces_has_no_steps
        covers all_steps > no_pieces:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        none = steps.is_empty()


-- A lone man in the open steps to either square ahead of him.
test a_lone_man_steps_both_ways_ahead
        covers all_steps > placed > direction'many > open:
    given:
        game = arranged([man(side.red, 5, 2)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        two_steps = (steps.count() == 2)
        up_left = (steps[0].path == [square(row: 4, column: 1)])
        up_right = (steps[1].path == [square(row: 4, column: 3)])


-- A man on the edge of the board cannot step off it.
test a_man_on_the_edge_steps_only_inward
        covers all_steps > placed > direction'many > blocked'1:
    given:
        game = arranged([man(side.red, 5, 0)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        one_step = (steps.count() == 1)
        inward = (steps[0].path == [square(row: 4, column: 1)])


-- A man cannot step onto a square a piece stands on.
test a_man_with_both_squares_ahead_taken_has_no_step
        covers all_steps > placed > direction'many > blocked'2:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 1), man(side.white, 4, 3)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        none = steps.is_empty()


-- Each of a player's men steps where it may, the first piece's steps first.
test each_of_several_men_steps
        covers all_steps > placed'many > direction'many > open:
    given:
        game = arranged([man(side.red, 5, 2), man(side.red, 7, 6)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        four_steps = (steps.count() == 4)
        the_upper_man_first = (steps[0].from == square(row: 5, column: 2))
        the_lower_man_last = (steps[3].from == square(row: 7, column: 6))


-- Men on the edge of the board each step only inward.
test men_on_the_edge_step_only_inward
        covers all_steps > placed'many > direction'many > blocked'1:
    given:
        game = arranged([man(side.red, 5, 0), man(side.red, 7, 0)], to_move: side.red)
    when:
        steps = all_steps(game)
    then:
        two_steps = (steps.count() == 2)
        the_upper_inward = (steps[0].path == [square(row: 4, column: 1)])
        the_lower_inward = (steps[1].path == [square(row: 6, column: 1)])


-- At the start, red's men behind the front row are blocked by their own, so only the front row steps.
test men_behind_the_front_row_are_blocked_at_the_start
        covers all_steps > placed'many > direction'many > blocked'2:
    given:
        game = new_game(levels.all[0])
    when:
        steps = all_steps(game)
    then:
        seven = (steps.count() == 7)
        from_the_front_row = (steps[6].from.row == 5)


-- A player with no pieces on the board has no jumps.
test a_player_with_no_pieces_has_no_jumps
        covers all_jumps > no_pieces:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        jumps = all_jumps(game)
    then:
        none = jumps.is_empty()


-- A lone man jumps the other player's piece ahead of him.
test a_lone_man_jumps_the_piece_ahead
        covers all_jumps > placed:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 3)], to_move: side.red)
    when:
        jumps = all_jumps(game)
    then:
        one_jump = (jumps.count() == 1)
        landed_beyond = (jumps[0].path == [square(row: 3, column: 4)])


-- Of several men, only those with a piece to jump have jumps.
test only_the_men_that_can_jump_have_jumps
        covers all_jumps > placed'many:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 3), man(side.red, 5, 6)], to_move: side.red)
    when:
        jumps = all_jumps(game)
    then:
        one_jump = (jumps.count() == 1)
        from_the_jumper = (jumps[0].from == square(row: 5, column: 2))


-- A jump with no jump after it is a whole move.
test a_jump_with_none_after_it_ends_the_chain
        covers chains > direction'many > can_jump > ends_here:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 3)], to_move: side.red)
    when:
        found = chains(game, piece(owner: side.red, king: false), square(row: 5, column: 2), square(row: 5, column: 2), [], [])
    then:
        one_move = (found.count() == 1)
        one_landing = (found[0].path == [square(row: 3, column: 4)])
        one_capture = (found[0].captures == [square(row: 4, column: 3)])


-- A piece that can jump again after a jump must go on, and the whole chain is one move.
test a_chain_of_jumps_is_one_move
        covers chains > direction'many > can_jump > goes_on:
    given:
        game = arranged([man(side.red, 6, 1), man(side.white, 5, 2), man(side.white, 3, 4)], to_move: side.red)
    when:
        found = chains(game, piece(owner: side.red, king: false), square(row: 6, column: 1), square(row: 6, column: 1), [], [])
    then:
        one_move = (found.count() == 1)
        two_landings = (found[0].path == [square(row: 4, column: 3), square(row: 2, column: 5)])
        two_captures = (found[0].captures == [square(row: 5, column: 2), square(row: 3, column: 4)])


-- A man with no piece of the other player's next to him has no chain.
test a_man_with_nothing_to_jump_has_no_chain
        covers chains > direction'many > cannot_jump'1:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 2, 7)], to_move: side.red)
    when:
        found = chains(game, piece(owner: side.red, king: false), square(row: 5, column: 2), square(row: 5, column: 2), [], [])
    then:
        none = found.is_empty()


-- A king that has jumped a piece cannot jump it again on the way back, though the square it started from is free.
test a_piece_is_not_jumped_twice
        covers chains > direction'many > cannot_jump'2:
    given:
        game = arranged([king(side.red, 5, 2), man(side.white, 4, 3)], to_move: side.red)
    when:
        found = chains(game, piece(owner: side.red, king: true), square(row: 5, column: 2), square(row: 3, column: 4), [square(row: 3, column: 4)], [square(row: 4, column: 3)])
    then:
        no_further_jump = found.is_empty()


-- A man that jumps onto the far row is crowned, and his chain ends there.
test a_man_crowned_by_a_jump_stops
        covers chains_after > crowned:
    given:
        game = arranged([man(side.red, 2, 1), man(side.white, 1, 2), man(side.white, 1, 4)], to_move: side.red)
    when:
        found = chains_after(game, piece(owner: side.red, king: false), square(row: 2, column: 1), square(row: 0, column: 3), [square(row: 0, column: 3)], [square(row: 1, column: 2)])
    then:
        ends_there = found.is_empty()


-- A king that lands on the far row jumps on from there.
test a_king_on_the_far_row_jumps_on
        covers chains_after > uncrowned'1:
    given:
        game = arranged([king(side.red, 2, 1), man(side.white, 1, 2), man(side.white, 1, 4)], to_move: side.red)
    when:
        found = chains_after(game, piece(owner: side.red, king: true), square(row: 2, column: 1), square(row: 0, column: 3), [square(row: 0, column: 3)], [square(row: 1, column: 2)])
    then:
        one_more_jump = (found.count() == 1)
        on_to_the_next = (found[0].path == [square(row: 0, column: 3), square(row: 2, column: 5)])


-- A white man that lands short of the bottom row jumps on.
test a_white_man_short_of_the_bottom_row_jumps_on
        covers chains_after > uncrowned'2lt:
    given:
        game = arranged([man(side.white, 1, 2), man(side.red, 2, 3), man(side.red, 4, 5)], to_move: side.white)
    when:
        found = chains_after(game, piece(owner: side.white, king: false), square(row: 1, column: 2), square(row: 3, column: 4), [square(row: 3, column: 4)], [square(row: 2, column: 3)])
    then:
        one_more_jump = (found.count() == 1)
        on_to_the_next = (found[0].path == [square(row: 3, column: 4), square(row: 5, column: 6)])


-- A red man that lands short of the top row jumps on.
test a_red_man_short_of_the_top_row_jumps_on
        covers chains_after > uncrowned'2gt:
    given:
        game = arranged([man(side.red, 6, 1), man(side.white, 5, 2), man(side.white, 3, 4)], to_move: side.red)
    when:
        found = chains_after(game, piece(owner: side.red, king: false), square(row: 6, column: 1), square(row: 4, column: 3), [square(row: 4, column: 3)], [square(row: 5, column: 2)])
    then:
        one_more_jump = (found.count() == 1)
        on_to_the_next = (found[0].path == [square(row: 4, column: 3), square(row: 2, column: 5)])


-- A piece may jump the other player's piece onto an empty square beyond it.
test a_piece_may_jump_onto_an_empty_square
        covers jumpable > inside:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 4, 3)], to_move: side.red)
    when:
        can = jumpable(game, piece(owner: side.red, king: false), square(row: 5, column: 2), square(row: 4, column: 3), square(row: 3, column: 4))
    then:
        it_may = can


-- A piece may not jump onto a square off the board.
test a_piece_may_not_jump_off_the_board
        covers jumpable > outside:
    given:
        game = arranged([man(side.red, 1, 2), man(side.white, 0, 3)], to_move: side.red)
    when:
        can = jumpable(game, piece(owner: side.red, king: false), square(row: 1, column: 2), square(row: 0, column: 3), square(row: -1, column: 4))
    then:
        it_may_not = (not can)


-- An empty square holds no piece of either player's.
test an_empty_square_belongs_to_no_one
        covers belongs_to > held.empty:
    given:
        game = arranged([man(side.white, 4, 3)], to_move: side.red)
    when:
        holds = belongs_to(game, square(row: 3, column: 4), side.white)
    then:
        not_white = (not holds)


-- A square with a piece on it holds a piece of that piece's owner.
test a_square_belongs_to_the_owner_of_its_piece
        covers belongs_to > held.piece:
    given:
        game = arranged([man(side.white, 4, 3)], to_move: side.red)
    when:
        holds = belongs_to(game, square(row: 4, column: 3), side.white)
    then:
        white = holds


-- Every piece on the board is found, row by row from the top left.
test every_piece_is_found_with_its_square
        covers placed_pieces > row'many > column'many > held.piece:
    given:
        game = new_game(levels.all[0])
    when:
        found = placed_pieces(game)
    then:
        twenty_four = (found.count() == 24)
        a_white_man_first = (found[0] == man(side.white, 0, 1))
        a_red_man_last = (found[23] == man(side.red, 7, 6))


-- On an empty board no piece is found.
test an_empty_board_has_no_pieces
        covers placed_pieces > row'many > column'many > held.empty:
    given:
        game = arranged([], to_move: side.red)
    when:
        found = placed_pieces(game)
    then:
        none = found.is_empty()


-- On an empty board the player to move has no pieces.
test a_player_on_an_empty_board_has_no_pieces
        covers own_pieces > no_pieces:
    given:
        game = arranged([], to_move: side.red)
    when:
        found = own_pieces(game)
    then:
        none = found.is_empty()


-- A lone piece of the player to move is theirs.
test a_lone_piece_is_its_players_own
        covers own_pieces > placed > mine:
    given:
        game = arranged([man(side.red, 4, 3)], to_move: side.red)
    when:
        found = own_pieces(game)
    then:
        the_one = (found == [man(side.red, 4, 3)])


-- A lone piece of the other player's is not the player to move's.
test a_lone_piece_of_the_other_player_is_not_own
        covers own_pieces > placed > theirs:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        found = own_pieces(game)
    then:
        none = found.is_empty()


-- Several pieces of the player to move are all theirs, in order.
test several_pieces_of_the_player_are_own
        covers own_pieces > placed'many > mine:
    given:
        game = arranged([man(side.red, 5, 0), king(side.red, 5, 2)], to_move: side.red)
    when:
        found = own_pieces(game)
    then:
        both = (found == [man(side.red, 5, 0), king(side.red, 5, 2)])


-- Of the pieces on the board, the other player's are left out.
test the_other_players_pieces_are_left_out
        covers own_pieces > placed'many > theirs:
    given:
        game = arranged([man(side.white, 2, 1), man(side.red, 5, 0)], to_move: side.red)
    when:
        found = own_pieces(game)
    then:
        only_red = (found == [man(side.red, 5, 0)])


-- A king moves every way, backwards as well as forwards.
test a_king_moves_every_way
        covers directions_of > moving.king:
    when:
        found = directions_of(piece(owner: side.red, king: true))
    then:
        all = (found == directions.all)


-- A red man moves up the board.
test a_red_man_moves_up
        covers directions_of > moving.red:
    when:
        found = directions_of(piece(owner: side.red, king: false))
    then:
        up = (found == directions.up)


-- A white man moves down the board.
test a_white_man_moves_down
        covers directions_of > moving.white:
    when:
        found = directions_of(piece(owner: side.white, king: false))
    then:
        down = (found == directions.down)


-- Red's men are crowned on the top row.
test red_is_crowned_on_the_top_row
        covers far_row > heading_up:
    when:
        row = far_row(side.red)
    then:
        top = (row == 0)


-- White's men are crowned on the bottom row.
test white_is_crowned_on_the_bottom_row
        covers far_row > heading_down:
    when:
        row = far_row(side.white)
    then:
        bottom = (row == 7)


-- The piece on a square is found.
test the_piece_on_a_square_is_found
        covers piece_at:
    given:
        game = arranged([king(side.white, 4, 3)], to_move: side.red)
    when:
        held = piece_at(game, square(row: 4, column: 3))
    then:
        a_white_king = (held == optional.some(piece(owner: side.white, king: true)))


-- Squares are numbered from 1, row by row from the top left.
test squares_are_numbered_row_by_row
        covers number_of:
    when:
        number = number_of(square(row: 1, column: 2))
    then:
        eleventh = (number == 11)


-- The bottom right corner is on the board.
test a_corner_is_on_the_board
        covers on_board:
    when:
        inside = on_board(square(row: 7, column: 7))
    then:
        on = inside


-- With one move and almost no checks, the search looks one move ahead, runs out at once, and makes it.
test with_one_move_and_no_checks_the_search_runs_out_at_once
        covers computer_move > deepening > candidate > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(1)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        the_only_move = (chosen.path == [square(row: 3, column: 6)])


-- With two moves and almost no checks, the search looks one move ahead, runs out at once, and makes one.
test with_two_moves_and_no_checks_the_search_runs_out_at_once
        covers computer_move > deepening > candidate'many > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(1)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        a_white_move = legal_moves(game).contains(chosen)


-- With one move and two checks, the search finishes the first depth before it runs out.
test with_one_move_and_two_checks_the_search_finishes_a_depth
        covers computer_move > deepening'many'1lt > candidate > finished:
    given:
        mutable game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(2)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        the_only_move = (chosen.path == [square(row: 3, column: 6)])


-- With one move and a few checks, the search runs out partway to its deepest.
test with_one_move_and_a_few_checks_the_search_runs_out_partway
        covers computer_move > deepening'many'1lt > candidate > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(24)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        the_only_move = (chosen.path == [square(row: 3, column: 6)])


-- With two moves and a few checks, the search finishes some depths before it runs out.
test with_two_moves_and_a_few_checks_the_search_finishes_some_depths
        covers computer_move > deepening'many'1lt > candidate'many > finished:
    given:
        mutable game = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(46)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        a_white_move = legal_moves(game).contains(chosen)


-- With one move and enough checks, the search finishes its deepest depth, and stops there.
test with_one_move_and_enough_checks_the_search_finishes_its_deepest
        covers computer_move > deepening'many'1eq > candidate > finished:
    given:
        mutable game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(1100)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        the_only_move = (chosen.path == [square(row: 3, column: 6)])


-- With one move and almost enough checks, the search reaches its deepest depth, and runs out there.
test with_one_move_the_search_runs_out_at_its_deepest
        covers computer_move > deepening'many'1eq > candidate > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(800)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        the_only_move = (chosen.path == [square(row: 3, column: 6)])


-- With two moves and enough checks, the search finishes its deepest depth, and stops there.
test with_two_moves_and_enough_checks_the_search_finishes_its_deepest
        covers computer_move > deepening'many'1eq > candidate'many > finished:
    given:
        mutable game = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(2100)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        a_white_move = legal_moves(game).contains(chosen)


-- With two moves and almost enough checks, the search reaches its deepest depth, and runs out there.
test with_two_moves_the_search_runs_out_at_its_deepest
        covers computer_move > deepening'many'1eq > candidate'many > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        game.level := trial(1500)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        a_white_move = legal_moves(game).contains(chosen)


-- The computer does not step its last man where the person's can jump it, and looks ahead until its checks run out.
test the_computer_keeps_its_man_out_of_reach
        covers computer_move > deepening'many'1lt > candidate'many > ran_out:
    given:
        mutable game = arranged([man(side.white, 2, 3), man(side.red, 4, 5)], to_move: side.white)
        game.level := levels.all[1]
    inject:
            random = fake random(seed: 7)
    when:
        chosen = computer_move(game)
    then:
        out_of_reach = (chosen.path == [square(row: 3, column: 2)])


-- Looking no moves ahead, a position is worth its pieces, and costs no checks.
test looking_no_moves_ahead_a_position_is_worth_its_pieces
        covers searched > judged.leaf'1:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 1), man(side.white, 0, 7)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.red, 0, mutable left)
    then:
        worth = (value == -3)
        checks_left = (left == 10)


-- A position in which the player to move cannot move is lost to them, however far ahead the search looks.
test a_position_with_no_moves_is_worth_its_end
        covers searched > judged.leaf'2:
    given:
        game = arranged([man(side.red, 1, 0), man(side.white, 0, 1)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.red, 3, mutable left)
    then:
        worth = (value == -search.win_value)
        checks_left = (left == 10)


-- The search stops at a drawn position, which is worth nothing.
test the_search_stops_at_a_draw
        covers searched > judged.leaf'3eq:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
        mutable left = 10
    when:
        value = searched(game, side.red, 3, mutable left)
    then:
        worth = (value == 0)
        checks_left = (left == 10)


-- The search stops at a position past the quiet limit too.
test the_search_stops_past_a_draw
        covers searched > judged.leaf'3gt:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit + 1
        mutable left = 10
    when:
        value = searched(game, side.red, 3, mutable left)
    then:
        worth = (value == 0)
        checks_left = (left == 10)


-- With exactly as many checks left as a position has moves, the search gives up and leaves none.
test the_search_gives_up_with_exactly_the_checks_the_moves_need
        covers searched > judged.spent'eq:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 2, 7)], to_move: side.red)
        mutable left = 2
    when:
        value = searched(game, side.red, 3, mutable left)
    then:
        worth = (value == 0)
        checks_left = (left == 0)


-- With fewer checks left than a position has moves, the search gives up and leaves none.
test the_search_gives_up_with_fewer_checks_than_moves
        covers searched > judged.spent'gt:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 2, 7)], to_move: side.red)
        mutable left = 1
    when:
        value = searched(game, side.red, 3, mutable left)
    then:
        worth = (value == 0)
        checks_left = (left == 0)


-- The owner's only move is looked at, one check, and the position is worth what it leads to.
test the_only_move_of_the_owner_is_weighed
        covers searched > judged.inner > candidate > owners_turn:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 1), man(side.white, 0, 7)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.red, 1, mutable left)
    then:
        worth = (value == -3)
        checks_left = (left == 9)


-- The other player's only move is looked at, one check, and the position is worth what it leads to.
test the_only_move_of_the_other_player_is_weighed
        covers searched > judged.inner > candidate > others_turn:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 1), man(side.white, 0, 7)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.white, 1, mutable left)
    then:
        worth = (value == 3)
        checks_left = (left == 9)


-- On the owner's turn, a position is worth the best of what its moves lead to.
test the_owner_takes_the_best_of_its_moves
        covers searched > judged.inner > candidate'many > owners_turn:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 3, 0)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.red, 2, mutable left)
    then:
        -- Expected the step out of reach of white's man, after which the pieces are even.
        even = (value == 0)
        checks_spent = (left < 10)


-- On the other player's turn, a position is worth the worst for the owner of what its moves lead to.
test the_other_player_takes_the_worst_of_its_moves
        covers searched > judged.inner > candidate'many > others_turn:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 3, 0)], to_move: side.red)
        mutable left = 10
    when:
        value = searched(game, side.white, 2, mutable left)
    then:
        even = (value == 0)
        checks_spent = (left < 10)


-- With one move to choose from, it is chosen without playing games out.
test one_option_is_chosen_unweighed
        covers best_sampled > option > ranked.top > ties.settled:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = best_sampled(game, options, [0], 10)
    then:
        the_best = (chosen == options[0])


-- Of two moves, the first, weighed best, is chosen.
test the_first_of_two_weighed_best_is_chosen
        covers best_sampled > option'many > ranked.top > ties.settled:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = best_sampled(game, options, [5, 3], 10)
    then:
        the_best = (chosen == options[0])


-- Two moves weighed the same are weighed again by games played out, and with no checks for them, by their pieces.
test tied_moves_are_weighed_by_games_played_out
        covers best_sampled > option'many > ranked.top > ties.contested > option'many:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = best_sampled(game, options, [2, 2], 0)
    then:
        the_best = (chosen == options[0])


-- Of two moves, the second, weighed best, is chosen.
test the_second_of_two_weighed_best_is_chosen
        covers best_sampled > option'many > ranked.lower > ties.settled:
    given:
        game = arranged([man(side.red, 5, 2), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = best_sampled(game, options, [3, 5], 10)
    then:
        the_best = (chosen == options[1])


-- Moves weighed lower are left out; the two weighed best are weighed again by games played out.
test moves_weighed_lower_are_left_out_of_the_games
        covers best_sampled > option'many > ranked.lower > ties.contested > option'many:
    given:
        game = arranged([man(side.red, 5, 2), man(side.red, 5, 6), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    inject:
            random = fake random(seed: 7)
    when:
        chosen = best_sampled(game, options, [2, 1, 2, 1], 0)
    then:
        the_best = (chosen == options[0])


-- With no checks to play games out, a position is worth its pieces.
test without_checks_the_pieces_decide
        covers sampled_value > no_share:
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 1, 2)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        worth = sampled_value(game, side.red, 0)
    then:
        the_pieces = (worth == 2)


-- One check plays one game out, which makes no move, so it is worth the pieces.
test one_check_plays_one_game_of_no_moves
        covers sampled_value > some_share > sampling:
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 1, 2)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        worth = sampled_value(game, side.red, 1)
    then:
        the_pieces = (worth == 2)


-- Games are played out until the checks run out, and the position is worth their average.
test games_are_played_until_the_checks_run_out
        covers sampled_value > some_share > sampling'many:
    given:
        game = arranged([man(side.red, 1, 0), man(side.white, 0, 1)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        worth = sampled_value(game, side.red, 2)
    then:
        -- Expected two games, each lost at once.
        lost = (worth == -search.win_value)


-- A game played out for no moves ends at once, worth its pieces, having looked at one position.
test a_game_played_out_for_no_moves_ends_at_once
        covers playout > ended'1:
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 1, 2)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 0)
    then:
        ended = (played == played_out(value: 2, spent: 1))


-- A game played out from a position with no moves ends at once, lost.
test a_game_with_no_moves_ends_at_once
        covers playout > ended'2:
    given:
        game = arranged([man(side.red, 1, 0), man(side.white, 0, 1)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 5)
    then:
        ended = (played == played_out(value: -search.win_value, spent: 1))


-- A game played out from a drawn position ends at once, worth nothing.
test a_drawn_game_ends_at_once
        covers playout > ended'3eq:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 5)
    then:
        ended = (played == played_out(value: 0, spent: 1))


-- A game played out from past the quiet limit ends at once, worth nothing.
test a_game_past_the_draw_ends_at_once
        covers playout > ended'3gt:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit + 1
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 5)
    then:
        ended = (played == played_out(value: 0, spent: 1))


-- A game played out for one move makes it, and looks at two positions.
test a_game_played_out_for_one_move
        covers playout > going:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 1)
    then:
        ended = (played == played_out(value: 0, spent: 2))


-- A game played out for two moves makes one for each player, and looks at three positions.
test a_game_played_out_for_two_moves
        covers playout > going'many:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
    inject:
            random = fake random(seed: 7)
    when:
        played = playout(game, side.red, 2)
    then:
        ended = (played == played_out(value: 0, spent: 3))


-- A game the player cannot move in is worth less the win value.
test a_lost_game_is_worth_less_the_win_value
        covers value_for > judged.losing:
    given:
        game = arranged([man(side.red, 4, 3)], to_move: side.white)
    when:
        worth = value_for(game, side.white)
    then:
        valued = (worth == -search.win_value)


-- A game the other player cannot move in is worth the win value.
test a_won_game_is_worth_the_win_value
        covers value_for > judged.winning:
    given:
        game = arranged([man(side.red, 4, 3)], to_move: side.white)
    when:
        worth = value_for(game, side.red)
    then:
        valued = (worth == search.win_value)


-- A game drawn at the quiet limit is worth nothing.
test a_drawn_game_is_worth_nothing
        covers value_for > judged.drawn'eq:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
    when:
        worth = value_for(game, side.red)
    then:
        valued = (worth == 0)


-- A game past the quiet limit is worth nothing.
test a_game_past_the_draw_is_worth_nothing
        covers value_for > judged.drawn'gt:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit + 1
    when:
        worth = value_for(game, side.red)
    then:
        valued = (worth == 0)


-- A game being played is worth the player's pieces less the other player's.
test pieces_are_worth_their_rank
        covers value_for > judged.playing:
    given:
        game = arranged([king(side.red, 4, 3), man(side.red, 6, 1), man(side.white, 1, 2)], to_move: side.red)
    when:
        worth = value_for(game, side.red)
    then:
        -- Expected 5 for the king and 3 for the man, less 3 for white's man.
        ahead = (worth == 5)


-- An empty board holds nothing of any worth.
test an_empty_board_is_worth_nothing
        covers material > no_pieces:
    given:
        game = arranged([], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == 0)


-- A lone king is worth a king's value to his player.
test a_lone_king_is_worth_a_kings_value
        covers material > placed > counted.king:
    given:
        game = arranged([king(side.red, 4, 3)], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == search.king_value)


-- A lone man is worth a man's value to his player.
test a_lone_man_is_worth_a_mans_value
        covers material > placed > counted.man:
    given:
        game = arranged([man(side.red, 4, 3)], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == search.man_value)


-- A lone piece is worth nothing to the other player.
test a_lone_piece_is_worth_nothing_to_the_other_player
        covers material > placed > counted.other:
    given:
        game = arranged([king(side.red, 4, 3)], to_move: side.red)
    when:
        worth = material(game, side.white)
    then:
        valued = (worth == 0)


-- Two kings are worth two kings' values.
test kings_add_up
        covers material > placed'many > counted.king:
    given:
        game = arranged([king(side.red, 4, 3), king(side.red, 6, 5)], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == 2 * search.king_value)


-- Two men are worth two men's values.
test men_add_up
        covers material > placed'many > counted.man:
    given:
        game = arranged([man(side.red, 4, 3), man(side.red, 6, 5)], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == 2 * search.man_value)


-- The other player's pieces add nothing to a player's worth.
test the_other_players_pieces_add_nothing
        covers material > placed'many > counted.other:
    given:
        game = arranged([man(side.red, 4, 3), king(side.white, 1, 2), man(side.white, 1, 6)], to_move: side.red)
    when:
        worth = material(game, side.red)
    then:
        valued = (worth == search.man_value)


-- While it is the person's move, the banner asks for it.
test the_banner_asks_for_the_persons_move
        covers banner > shown.your_move:
    given:
        game = new_game(levels.all[2])
    when:
        lines = banner(game, 50)
    then:
        says = (lines == ["Your move   Club   Moves 0   Best 50", "Click a red piece, then a marked square. Escape quits."])


-- While it is the computer's move, the banner says it is thinking.
test the_banner_says_the_computer_is_thinking
        covers banner > shown.thinking:
    given:
        mutable game = new_game(levels.all[2])
        game.to_move := side.white
        game.turns := 1
    when:
        lines = banner(game, 50)
    then:
        says = (lines == ["The computer is thinking, at 2500 positions a move...", "Escape quits."])


-- Once the person has won, the banner gives the score.
test the_banner_gives_the_score_of_a_win
        covers banner > shown.won:
    given:
        mutable game = arranged([man(side.red, 4, 3)], to_move: side.white)
        game.turns := 59
    when:
        lines = banner(game, 2100)
    then:
        says = (lines == ["You won in 30 moves against Club! Score 2100   Best 2100", "Space chooses a new game. Escape quits."])


-- Once the computer has won, the banner says so, and scores nothing.
test the_banner_says_the_computer_won
        covers banner > shown.lost:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        lines = banner(game, 50)
    then:
        says = (lines == ["The computer won. Score 0   Best 50", "Space chooses a new game. Escape quits."])


-- Once the game is drawn, the banner says why, and scores nothing.
test the_banner_says_the_game_is_drawn
        covers banner > shown.drawn:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
    when:
        lines = banner(game, 50)
    then:
        says = (lines == ["A draw: 80 turns without a capture. Score 0   Best 50", "Space chooses a new game. Escape quits."])


-- Drawing a game writes its banner beneath the board.
test the_banner_is_written_beneath_the_board
        covers draw:
    given:
        game = arranged([], to_move: side.red)
    inject:
            window = fake window(frames: [])
    when:
        draw(game, optional.none, 0)
    then:
        banner_written = window.written().contains("The computer won. Score 0   Best 0")
        what_to_do_written = window.written().contains("Space chooses a new game. Escape quits.")


-- The levels are drawn, each with what it looks at, and the best score.
test the_levels_are_drawn_with_the_best_score
        covers draw_levels > level'many:
    inject:
            window = fake window(frames: [])
    when:
        draw_levels(2100)
    then:
        titled = window.written().contains("Checkers")
        the_easiest = window.written().contains("Beginner")
        the_hardest = window.written().contains("Looks at 250000 positions a move. Wins score x5.")
        best = window.written().contains("Best 2100")


-- A click on the board finds the square under it.
test a_click_on_the_board_finds_its_square
        covers square_at > inside:
    when:
        found = square_at(point(x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5))
    then:
        the_square = (found == optional.some(square(row: 5, column: 3)))


-- A click to the left of the board finds no square.
test a_click_left_of_the_board_finds_no_square
        covers square_at > outside'1:
    when:
        found = square_at(point(x: 5, y: 5))
    then:
        none = (found == optional.none)


-- A click below the board, on the bar, finds no square.
test a_click_below_the_board_finds_no_square
        covers square_at > outside'2:
    when:
        found = square_at(point(x: 100, y: 700))
    then:
        none = (found == optional.none)


-- A click on the first button finds the easiest level.
test a_click_on_the_first_button_finds_the_easiest_level
        covers level_at > level > pressed:
    when:
        found = level_at(point(x: 100, y: layout.button_top + 10))
    then:
        beginner = (found == optional.some(levels.all[0]))


-- A click on the second button finds the second level.
test a_click_on_the_second_button_finds_its_level
        covers level_at > level'many > pressed:
    when:
        found = level_at(point(x: 100, y: layout.button_top + layout.button_step + 10))
    then:
        casual = (found == optional.some(levels.all[1]))


-- A click beside the buttons finds no level.
test a_click_beside_the_buttons_finds_no_level
        covers level_at > level'many > missed'1:
    when:
        found = level_at(point(x: 595, y: layout.button_top + 10))
    then:
        none = (found == optional.none)


-- A click above the buttons finds no level.
test a_click_above_the_buttons_finds_no_level
        covers level_at > level'many > missed'2:
    when:
        found = level_at(point(x: 100, y: 10))
    then:
        none = (found == optional.none)


-- A square's top left corner is past the gap, a square's side for each step.
test a_square_has_its_corner_inside_the_gap
        covers corner_of:
    when:
        corner = corner_of(square(row: 1, column: 2))
    then:
        placed = (corner == point(x: 156, y: 84))


-- A square's centre is half a side in from its corner.
test a_square_has_its_centre_half_a_side_in
        covers centre_of:
    when:
        centre = centre_of(square(row: 1, column: 2))
    then:
        placed = (centre == point(x: 192, y: 120))


-- A light square is drawn light.
test a_light_square_is_light
        covers shade_of > looks.light:
    given:
        game = new_game(levels.all[0])
    when:
        shade = shade_of(game, square(row: 0, column: 0), optional.none)
    then:
        light = (shade == palette.light)


-- The square of the selected piece is marked.
test the_selected_square_is_marked
        covers shade_of > looks.selected:
    given:
        game = new_game(levels.all[0])
    when:
        shade = shade_of(game, square(row: 5, column: 2), optional.some(square(row: 5, column: 2)))
    then:
        selected = (shade == palette.selected)


-- A square the last move started from is marked.
test the_last_move_is_marked
        covers shade_of > looks.last:
    given:
        mutable game = arranged([man(side.red, 4, 1)], to_move: side.white)
        game.last := optional.some(move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 0), path: [square(row: 4, column: 1)], captures: []))
    when:
        shade = shade_of(game, square(row: 5, column: 0), optional.none)
    then:
        last = (shade == palette.last)


-- A dark square, neither selected nor part of the last move, is drawn dark.
test a_dark_square_is_dark
        covers shade_of > looks.dark:
    given:
        game = new_game(levels.all[0])
    when:
        shade = shade_of(game, square(row: 5, column: 0), optional.none)
    then:
        dark = (shade == palette.dark)


-- Before any move is made, no square is part of the last move.
test before_any_move_no_square_is_part_of_the_last
        covers in_last_move > made.none:
    given:
        game = new_game(levels.all[0])
    when:
        part = in_last_move(game, square(row: 5, column: 0))
    then:
        not_part = (not part)


-- The square the last move landed on is part of it.
test the_square_landed_on_is_part_of_the_last_move
        covers in_last_move > made.some:
    given:
        mutable game = arranged([man(side.red, 4, 1)], to_move: side.white)
        game.last := optional.some(move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 0), path: [square(row: 4, column: 1)], captures: []))
    when:
        part = in_last_move(game, square(row: 4, column: 1))
    then:
        landed_on = part


-- A player with no moves has nowhere to go.
test a_player_with_no_moves_has_nowhere_to_go
        covers landings > no_moves:
    given:
        game = arranged([], to_move: side.red)
    when:
        found = landings(game, optional.some(square(row: 5, column: 0)))
    then:
        nowhere = found.is_empty()


-- With one move to make, its piece shows where it goes.
test the_only_move_shows_where_it_goes
        covers landings > candidate > from_selected:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        found = landings(game, optional.some(square(row: 5, column: 0)))
    then:
        one_square = (found == [square(row: 4, column: 1)])


-- With one move to make, a piece that does not make it shows nothing.
test another_piece_shows_nowhere_to_go
        covers landings > candidate > from_elsewhere:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        found = landings(game, optional.some(square(row: 0, column: 7)))
    then:
        nowhere = found.is_empty()


-- The selected piece shows each square it may move to.
test the_selected_piece_shows_where_it_may_go
        covers landings > candidate'many > from_selected:
    given:
        game = new_game(levels.all[0])
    when:
        found = landings(game, optional.some(square(row: 5, column: 2)))
    then:
        two_squares = (found == [square(row: 4, column: 1), square(row: 4, column: 3)])


-- With no piece selected, nothing is shown.
test with_none_selected_nothing_is_shown
        covers landings > candidate'many > from_elsewhere:
    given:
        game = new_game(levels.all[0])
    when:
        found = landings(game, optional.none)
    then:
        nowhere = found.is_empty()


-- The board is drawn square by square, with no text.
test the_board_is_drawn_in_squares
        covers draw_board > row'many > column'many:
    given:
        game = new_game(levels.all[0])
    inject:
            window = fake window(frames: [])
    when:
        draw_board(game, optional.none)
    then:
        nothing_written = window.written().is_empty()
        every_square = (window.fills() == 64)


-- An empty board has no pieces to draw.
test an_empty_board_has_no_pieces_to_draw
        covers draw_pieces > no_pieces:
    given:
        game = arranged([], to_move: side.red)
    inject:
            window = fake window(frames: [])
    when:
        draw_pieces(game)
    then:
        nothing_written = window.written().is_empty()
        no_discs = (window.circles() == 0)


-- A lone piece is drawn, with no text.
test a_lone_piece_is_drawn
        covers draw_pieces > placed:
    given:
        game = arranged([man(side.red, 5, 0)], to_move: side.red)
    inject:
            window = fake window(frames: [])
    when:
        draw_pieces(game)
    then:
        nothing_written = window.written().is_empty()
        three_discs = (window.circles() == 3)


-- Every piece is drawn, with no text.
test every_piece_is_drawn
        covers draw_pieces > placed'many:
    given:
        game = new_game(levels.all[0])
    inject:
            window = fake window(frames: [])
    when:
        draw_pieces(game)
    then:
        nothing_written = window.written().is_empty()
        three_discs_each = (window.circles() == 72)


-- With no piece selected, no square is marked.
test with_none_selected_no_square_is_marked
        covers draw_marks > no_marks:
    given:
        game = new_game(levels.all[0])
    inject:
            window = fake window(frames: [])
    when:
        draw_marks(game, optional.none)
    then:
        nothing_written = window.written().is_empty()
        no_marks = (window.circles() == 0)


-- The one square the selected piece may move to is marked.
test the_one_square_of_the_only_move_is_marked
        covers draw_marks > landing:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    inject:
            window = fake window(frames: [])
    when:
        draw_marks(game, optional.some(square(row: 5, column: 0)))
    then:
        nothing_written = window.written().is_empty()
        one_mark = (window.circles() == 1)


-- Each square the selected piece may move to is marked.
test each_square_the_selected_piece_may_go_to_is_marked
        covers draw_marks > landing'many:
    given:
        game = new_game(levels.all[0])
    inject:
            window = fake window(frames: [])
    when:
        draw_marks(game, optional.some(square(row: 5, column: 2)))
    then:
        nothing_written = window.written().is_empty()
        two_marks = (window.circles() == 2)


-- A piece is drawn as discs, with no text.
test a_piece_is_drawn_as_discs
        covers draw_piece:
    inject:
            window = fake window(frames: [])
    when:
        draw_piece(man(side.red, 5, 0))
    then:
        nothing_written = window.written().is_empty()
        three_discs = (window.circles() == 3)


-- A red king is drawn in red's colours, with a crown at the centre.
test a_red_king_is_red_with_a_crown
        covers colours_of > painted.red_king:
    when:
        colours = colours_of(piece(owner: side.red, king: true))
    then:
        crowned_red = (colours == [palette.red, palette.red_ring, palette.crown])


-- A red man is drawn in red's colours.
test a_red_man_is_red
        covers colours_of > painted.red_man:
    when:
        colours = colours_of(piece(owner: side.red, king: false))
    then:
        plain_red = (colours == [palette.red, palette.red_ring, palette.red])


-- A white king is drawn in white's colours, with a crown at the centre.
test a_white_king_is_white_with_a_crown
        covers colours_of > painted.white_king:
    when:
        colours = colours_of(piece(owner: side.white, king: true))
    then:
        crowned_white = (colours == [palette.white, palette.white_ring, palette.crown])


-- A white man is drawn in white's colours.
test a_white_man_is_white
        covers colours_of > painted.white_man:
    when:
        colours = colours_of(piece(owner: side.white, king: false))
    then:
        plain_white = (colours == [palette.white, palette.white_ring, palette.white])


-- Escape on the levels ends play before a game.
test escape_at_the_levels_ends_play
        covers play > open > picked.none:
    inject:
            window = fake window(frames: [frame(keys: [key.escape])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)
        levels_shown = window.written().contains("Checkers")


-- The player clicks a level, selects a man and moves him, and the computer replies; then Escape ends play.
test a_level_is_chosen_and_a_move_made
        covers play > open > picked.level:
    inject:
            window = fake window(frames: [
                frame(clicks: [point(x: 100, y: 10)]),
                frame(clicks: [point(x: 100, y: layout.button_top + 10)]),
                clicking(row: 5, column: 2),
                clicking(row: 4, column: 3),
                frame(),
                frame(keys: [key.escape]),
            ])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)
        thinking = window.written().contains("The computer is thinking, at 25 positions a move...")
        replied = window.written().contains("Your move   Beginner   Moves 1   Best 0")


-- A window closed as a game starts ends that game, and then play.
test a_window_closed_as_a_game_starts_ends_play
        covers play > open'many > picked.none:
    inject:
            window = fake window(frames: [frame(clicks: [point(x: 100, y: layout.button_top + 10)])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)


-- A level chosen starts a game at that level, shown until the window is closed.
test a_game_is_played_at_the_level_chosen
        covers play > open'many > picked.level:
    inject:
            window = fake window(frames: [frame(clicks: [point(x: 100, y: layout.button_top + 10)]), frame()])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)
        at_the_level = window.written().contains("Your move   Beginner   Moves 0   Best 0")


-- A window closed at once chooses no level.
test a_window_closed_at_once_chooses_no_level
        covers choose_level > closed_at_once'3:
    inject:
            window = fake window(frames: [])
            forever = fake forever
    when:
        found = choose_level(0)
    then:
        none = (found == optional.none)


-- A click on a level's button in the first frame chooses that level.
test a_click_on_a_button_chooses_its_level
        covers choose_level > choosing:
    inject:
            window = fake window(frames: [frame(clicks: [point(x: 100, y: layout.button_top + 10)])])
            forever = fake forever
    when:
        found = choose_level(0)
    then:
        beginner = (found == optional.some(levels.all[0]))
        levels_shown = window.written().contains("Beginner")


-- The levels are shown, frame after frame, until a button is clicked.
test the_levels_are_shown_until_one_is_clicked
        covers choose_level > choosing'many:
    inject:
            window = fake window(frames: [frame(clicks: [point(x: 100, y: 10)]), frame(clicks: [point(x: 100, y: layout.button_top + layout.button_step + 10)])])
            forever = fake forever
    when:
        found = choose_level(0)
    then:
        casual = (found == optional.some(levels.all[1]))
        two_frames = (window.frames_shown() == 2)


-- A window closed at once ends the game with its score.
test a_window_closed_at_once_still_scores_the_game
        covers play_game > closed_at_once'3:
    given:
        mutable won = arranged([man(side.red, 4, 3)], to_move: side.white)
        won.turns := 59
    inject:
            window = fake window(frames: [])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(won, 0)
    then:
        scored = (ended == ending(points: 2100, quitting: false))


-- Once a game has ended, Space ends it with its score, to choose another.
test space_after_a_game_chooses_another
        covers play_game > playing > moment.over:
    given:
        mutable won = arranged([man(side.red, 4, 3)], to_move: side.white)
        won.turns := 59
    inject:
            window = fake window(frames: [frame(keys: [key.space])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(won, 0)
    then:
        scored = (ended == ending(points: 2100, quitting: false))


-- On the computer's turn, it moves in a frame of its own.
test the_computer_moves_in_a_frame
        covers play_game > playing > moment.computers_turn:
    given:
        game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
    inject:
            window = fake window(frames: [frame()])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(game, 0)
    then:
        not_quitting = (ended == ending(points: 0, quitting: false))
        your_move = window.written().contains("Your move   Club   Moves 1   Best 0")


-- On the person's turn, a frame shows the game and waits for their clicks.
test the_player_has_a_frame_to_act_in
        covers play_game > playing > moment.players_turn:
    given:
        game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.red)
    inject:
            window = fake window(frames: [frame()])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(game, 0)
    then:
        not_quitting = (ended == ending(points: 0, quitting: false))
        your_move = window.written().contains("Your move   Club   Moves 0   Best 0")


-- Escape during the person's turn ends the game, asking to stop playing.
test escape_ends_a_game
        covers play_game > playing > moment.players_turn:
    given:
        game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.red)
    inject:
            window = fake window(frames: [frame(keys: [key.escape])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(game, 0)
    then:
        quitting = (ended == ending(points: 0, quitting: true))


-- An ended game waits, frame after frame, until the player presses Space.
test an_ended_game_waits_for_space
        covers play_game > playing'many > moment.over:
    given:
        mutable won = arranged([man(side.red, 4, 3)], to_move: side.white)
        won.turns := 59
    inject:
            window = fake window(frames: [frame(), frame(keys: [key.space])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(won, 0)
    then:
        scored = (ended == ending(points: 2100, quitting: false))


-- After the computer moves, it is the person's turn.
test the_game_goes_on_after_the_computer_moves
        covers play_game > playing'many > moment.computers_turn:
    given:
        game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
    inject:
            window = fake window(frames: [frame(), frame()])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(game, 0)
    then:
        not_quitting = (ended == ending(points: 0, quitting: false))
        two_frames = (window.frames_shown() == 2)


-- The person selects a man in one frame and moves him in the next, and then the computer thinks.
test the_player_selects_and_moves_over_two_frames
        covers play_game > playing'many > moment.players_turn:
    given:
        game = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.red)
    inject:
            window = fake window(frames: [clicking(row: 7, column: 0), clicking(row: 6, column: 1)])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        ended = play_game(game, 0)
    then:
        not_quitting = (ended == ending(points: 0, quitting: false))
        thinking = window.written().contains("The computer is thinking, at 2500 positions a move...")


-- A click on a square the selected piece may move to makes the move, and selects nothing.
test a_click_where_the_selected_piece_may_go_moves_it
        covers clicked > judged.moving:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.some(square(row: 5, column: 2))
    when:
        clicked(mutable game, mutable selected, centre_of(square(row: 4, column: 3)))
    then:
        moved = (piece_at(game, square(row: 4, column: 3)) == optional.some(piece(owner: side.red, king: false)))
        nothing_selected = (selected == optional.none)
        white_to_move = (game.to_move == side.white)


-- A click on a piece of the player's that may move selects it.
test a_click_on_a_piece_that_may_move_selects_it
        covers clicked > judged.choosing:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.none
    when:
        clicked(mutable game, mutable selected, centre_of(square(row: 5, column: 2)))
    then:
        selected_it = (selected == optional.some(square(row: 5, column: 2)))
        red_still_to_move = (game.to_move == side.red)


-- A click on neither a piece that may move nor a square the selected piece may move to selects nothing.
test a_click_elsewhere_selects_nothing
        covers clicked > judged.elsewhere:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.some(square(row: 5, column: 2))
    when:
        clicked(mutable game, mutable selected, centre_of(square(row: 0, column: 0)))
    then:
        nothing_selected = (selected == optional.none)
        red_still_to_move = (game.to_move == side.red)


-- A click on a level's button picks that level.
test a_click_on_a_button_picks_its_level
        covers level_clicked > place > hit:
    when:
        found = level_clicked([point(x: 100, y: layout.button_top + 10)])
    then:
        beginner = (found == optional.some(levels.all[0]))


-- Of two clicks, the first on a button picks the level.
test the_first_click_on_a_button_counts
        covers level_clicked > place'many > hit:
    when:
        found = level_clicked([point(x: 100, y: 10), point(x: 100, y: layout.button_top + 10)])
    then:
        beginner = (found == optional.some(levels.all[0]))


-- With no clicks, no level is picked.
test no_clicks_pick_no_level
        covers level_clicked > no_clicks:
    when:
        found = level_clicked([])
    then:
        none = (found == optional.none)


-- A click off the buttons picks no level.
test a_click_off_the_buttons_picks_no_level
        covers level_clicked > place > miss:
    when:
        found = level_clicked([point(x: 100, y: 10)])
    then:
        none = (found == optional.none)


-- Clicks that all miss the buttons pick no level.
test clicks_that_all_miss_pick_no_level
        covers level_clicked > place'many > miss:
    when:
        found = level_clicked([point(x: 100, y: 10), point(x: 595, y: 10)])
    then:
        none = (found == optional.none)


-- With no clicks, the selection and the game stay as they were.
test no_clicks_change_nothing
        covers act_on_clicks > no_clicks:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.some(square(row: 5, column: 2))
    when:
        act_on_clicks(mutable game, mutable selected, [])
    then:
        still_selected = (selected == optional.some(square(row: 5, column: 2)))
        red_still_to_move = (game.to_move == side.red)


-- One click on a man that may move selects him.
test one_click_selects_a_man
        covers act_on_clicks > place:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.none
    when:
        act_on_clicks(mutable game, mutable selected, [centre_of(square(row: 5, column: 2))])
    then:
        selected_him = (selected == optional.some(square(row: 5, column: 2)))


-- Two clicks in one frame select a man, then move him.
test two_clicks_in_a_frame_make_a_move
        covers act_on_clicks > place'many:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.none
    when:
        act_on_clicks(mutable game, mutable selected, [centre_of(square(row: 5, column: 2)), centre_of(square(row: 4, column: 3))])
    then:
        moved = (piece_at(game, square(row: 4, column: 3)) == optional.some(piece(owner: side.red, king: false)))
        white_to_move = (game.to_move == side.white)


-- With one move in the game, it is found from its square to its square.
test the_only_move_is_found
        covers move_to > candidate > fits:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        found = move_to(game, optional.some(square(row: 5, column: 0)), optional.some(square(row: 4, column: 1)))
    then:
        found_it = (found == optional.some(move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 0), path: [square(row: 4, column: 1)], captures: [])))


-- A move among many is found from its square to its square.
test a_move_among_many_is_found
        covers move_to > candidate'many > fits:
    given:
        game = new_game(levels.all[0])
    when:
        found = move_to(game, optional.some(square(row: 5, column: 2)), optional.some(square(row: 4, column: 3)))
    then:
        found_it = (found == optional.some(move(piece: piece(owner: side.red, king: false), from: square(row: 5, column: 2), path: [square(row: 4, column: 3)], captures: [])))


-- With no moves in the game, none is found.
test with_no_moves_none_is_found
        covers move_to > no_moves:
    given:
        game = arranged([], to_move: side.red)
    when:
        found = move_to(game, optional.some(square(row: 5, column: 0)), optional.some(square(row: 4, column: 1)))
    then:
        no_move = (found == optional.none)


-- With one move in the game, none is found from another square.
test the_only_move_is_not_found_from_another_square
        covers move_to > candidate > differs'1:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        found = move_to(game, optional.some(square(row: 6, column: 1)), optional.some(square(row: 4, column: 1)))
    then:
        no_move = (found == optional.none)


-- With one move in the game, none is found from its square to another.
test the_only_move_is_not_found_to_another_square
        covers move_to > candidate > differs'2:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        found = move_to(game, optional.some(square(row: 5, column: 0)), optional.some(square(row: 3, column: 2)))
    then:
        no_move = (found == optional.none)


-- No move is found from a man who cannot move.
test no_move_is_found_from_a_blocked_man
        covers move_to > candidate'many > differs'1:
    given:
        game = new_game(levels.all[0])
    when:
        found = move_to(game, optional.some(square(row: 6, column: 1)), optional.some(square(row: 5, column: 0)))
    then:
        no_move = (found == optional.none)


-- No move is found to a square two steps away.
test no_move_is_found_two_squares_away
        covers move_to > candidate'many > differs'2:
    given:
        game = new_game(levels.all[0])
    when:
        found = move_to(game, optional.some(square(row: 5, column: 2)), optional.some(square(row: 3, column: 2)))
    then:
        no_move = (found == optional.none)


-- With one move in the game, its piece may move.
test the_piece_of_the_only_move_may_move
        covers may_move_from > candidate > movable:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        can = may_move_from(game, optional.some(square(row: 5, column: 0)))
    then:
        movable = can


-- At the start, a man of the front row may move.
test a_front_man_may_move
        covers may_move_from > candidate'many > movable:
    given:
        game = new_game(levels.all[0])
    when:
        can = may_move_from(game, optional.some(square(row: 5, column: 2)))
    then:
        movable = can


-- With no moves in the game, nothing may move.
test with_no_moves_nothing_may_move
        covers may_move_from > no_moves:
    given:
        game = arranged([], to_move: side.red)
    when:
        can = may_move_from(game, optional.some(square(row: 5, column: 0)))
    then:
        not_movable = (not can)


-- With one move in the game, no piece on another square may move.
test another_square_may_not_move
        covers may_move_from > candidate > unmovable:
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
    when:
        can = may_move_from(game, optional.some(square(row: 6, column: 1)))
    then:
        not_movable = (not can)


-- At the start, a man behind the front row may not move.
test a_blocked_man_may_not_move
        covers may_move_from > candidate'many > unmovable:
    given:
        game = new_game(levels.all[0])
    when:
        can = may_move_from(game, optional.some(square(row: 6, column: 1)))
    then:
        not_movable = (not can)
