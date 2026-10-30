--------------------------------------------------------------------------------------------------------------------------------
A game with only the given pieces on the board, at the Club level.

pieces: The pieces and their squares, as list<placed>.
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


-- A new game has twelve men for each player, on the dark squares of the three rows nearest them, and is being played.
test a_new_game_has_twelve_men_each
        covers new_game > row'many > column'many, starting_piece > placed.light, starting_piece > placed.white_men,
               starting_piece > placed.red_men'eq, starting_piece > placed.red_men'gt, starting_piece > placed.between,
               standing > judged.playing, all_steps > placed'many > direction'many > blocked'1,
               all_steps > placed'many > direction'many > blocked'2,
               placed_pieces > row'many > column'many > held.piece, placed_pieces > row'many > column'many > held.empty:
    when:
        started = new_game(levels.all[0])
        pieces = placed_pieces(started)
    then:
        twenty_four = (pieces.count() == 24)
        a_red_man = (piece_at(started, square(row: 5, column: 0)) == optional.some(piece(owner: side.red, king: false)))
        a_white_man = (piece_at(started, square(row: 2, column: 1)) == optional.some(piece(owner: side.white, king: false)))
        light_is_empty = (piece_at(started, square(row: 0, column: 0)) == optional.none)
        the_middle_is_empty = (piece_at(started, square(row: 3, column: 2)) == optional.none)
        red_to_move = (started.to_move == side.red)
        playing = (standing(started) == outcome.playing)


-- Each player has seven steps to choose from at the start, red's up the board and white's down it.
test the_first_moves_are_steps:
    given:
        mutable game = new_game(levels.all[0])
    when:
        reds = legal_moves(game)
        apply(mutable game, reds[0])
        whites = legal_moves(game)
    then:
        seven_for_red = (reds.count() == 7)
        seven_for_white = (whites.count() == 7)
        red_goes_up = (reds[0].path.last().row == reds[0].from.row - 1)
        white_goes_down = (whites[0].path.last().row == whites[0].from.row + 1)
        white_to_move = (game.to_move == side.white)
        one_quiet_turn = (game.quiet == 1)


-- A player who can capture must, and the piece jumped over is taken off the board.
test a_capture_must_be_taken
        covers all_jumps > placed'many, chains_after > uncrowned'2gt:
    given:
        mutable game = arranged([man(side.red, 5, 2), man(side.white, 4, 3), man(side.red, 5, 6)], to_move: side.red)
        game.quiet := 7
    when:
        moves = legal_moves(game)
        apply(mutable game, moves[0])
    then:
        only_the_jump = (moves.count() == 1)
        jumped_over = (moves[0].captures == [square(row: 4, column: 3)])
        taken = (piece_at(game, square(row: 4, column: 3)) == optional.none)
        landed = (piece_at(game, square(row: 3, column: 4)) == optional.some(piece(owner: side.red, king: false)))
        no_longer_quiet = (game.quiet == 0)


-- A piece that can jump again after a jump must go on, and the whole chain is one move.
test a_chain_of_jumps_is_one_move
        covers chains > direction'many > can_jump > goes_on:
    given:
        game = arranged([man(side.red, 6, 1), man(side.white, 5, 2), man(side.white, 3, 4)], to_move: side.red)
    when:
        moves = legal_moves(game)
    then:
        one_move = (moves.count() == 1)
        two_landings = (moves[0].path == [square(row: 4, column: 3), square(row: 2, column: 5)])
        two_captures = (moves[0].captures == [square(row: 5, column: 2), square(row: 3, column: 4)])


-- A man that jumps onto the far row is crowned, and his move ends there, though a king could jump on.
test a_man_crowned_by_a_jump_stops
        covers legal_moves > must_jump, apply > captured, chains > direction'many > can_jump > ends_here,
               chains_after > crowned, belongs_to > held.piece, far_row > heading_up:
    given:
        mutable game = arranged([man(side.red, 2, 1), man(side.white, 1, 2), man(side.white, 1, 4)], to_move: side.red)
    when:
        moves = legal_moves(game)
        apply(mutable game, moves[0])
    then:
        one_jump = (moves[0].path == [square(row: 0, column: 3)])
        crowned = (piece_at(game, square(row: 0, column: 3)) == optional.some(piece(owner: side.red, king: true)))


-- A king steps backwards as well as forwards.
test a_king_moves_both_ways
        covers legal_moves > no_jump, other > current.red, all_steps > placed > direction'many > open,
               all_jumps > placed, chains > direction'many > cannot_jump'1, jumpable > inside, belongs_to > held.empty,
               directions_of > moving.king, on_board:
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 0, 1)], to_move: side.red)
    when:
        moves = legal_moves(game)
    then:
        four_ways = (moves.count() == 4)


-- White's men are crowned on the bottom row.
test white_is_crowned_on_the_bottom_row
        covers apply > quiet, other > current.white, directions_of > moving.white, far_row > heading_down:
    given:
        mutable game = arranged([man(side.white, 6, 1), man(side.red, 0, 7)], to_move: side.white)
    when:
        moves = legal_moves(game)
        apply(mutable game, moves[0])
    then:
        crowned = (piece_at(game, moves[0].path.last()) == optional.some(piece(owner: side.white, king: true)))
        red_to_move = (game.to_move == side.red)


-- The person wins when the computer cannot move; a win scores less for each move it took, but never less than the
-- least win, and all of it times the level's weight.
test a_win_scores_by_level_and_moves
        covers standing > judged.won, score > won, player_moves:
    given:
        mutable quick = arranged([man(side.red, 4, 3)], to_move: side.white)
        quick.turns := 59
        mutable slow = arranged([man(side.red, 4, 3)], to_move: side.white)
        slow.turns := 299
    when:
        quick_points = score(quick)
        slow_points = score(slow)
    then:
        won = (standing(quick) == outcome.won)

        -- Expected 1000 points, less 300 for 30 moves, times 3 at the Club level.
        quick_score = (quick_points == 2100)

        -- Expected the least win, 100 points, times 3.
        slow_score = (slow_points == 300)


-- The person loses when they cannot move, and a loss scores nothing.
test a_loss_scores_nothing
        covers standing > judged.lost, score > unfinished, all_steps > no_pieces, all_jumps > no_pieces,
               own_pieces > placed > theirs:
    given:
        game = arranged([man(side.white, 3, 2)], to_move: side.red)
    when:
        reached = standing(game)
        points = score(game)
    then:
        lost = (reached == outcome.lost)
        nothing = (points == 0)


-- Too many turns without a capture draw the game.
test quiet_turns_draw_the_game
        covers standing > judged.drawn'eq, standing > judged.drawn'gt:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
        mutable later = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        later.quiet := game_rules.quiet_limit + 1
    when:
        reached = standing(game)
        reached_later = standing(later)
    then:
        drawn = (reached == outcome.drawn)
        still_drawn = (reached_later == outcome.drawn)


-- A position is worth the pieces of each side, kings more than men.
test pieces_are_worth_their_rank
        covers all_steps > placed'many > direction'many > open, value_for > judged.playing:
    given:
        game = arranged([king(side.red, 4, 3), man(side.red, 6, 1), man(side.white, 1, 2)], to_move: side.red)
    when:
        for_red = value_for(game, side.red)
        for_white = value_for(game, side.white)
    then:
        -- Expected 5 for the king and 3 for the man, less 3 for white's man.
        red_ahead = (for_red == 5)
        white_behind = (for_white == -5)


-- A game that is over is worth the win value to the winner, less it to the loser, and nothing when drawn.
test an_ended_game_is_worth_its_end
        covers value_for > judged.losing, value_for > judged.winning, value_for > judged.drawn'eq,
               value_for > judged.drawn'gt:
    given:
        stuck = arranged([man(side.red, 4, 3)], to_move: side.white)
        mutable drawn = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        drawn.quiet := game_rules.quiet_limit
        mutable later = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        later.quiet := game_rules.quiet_limit + 1
    when:
        for_white = value_for(stuck, side.white)
        for_red = value_for(stuck, side.red)
        for_either = value_for(drawn, side.red)
        for_either_later = value_for(later, side.red)
    then:
        white_lost = (for_white == -search.win_value)
        red_won = (for_red == search.win_value)
        nothing_for_a_draw = (for_either == 0)
        nothing_later = (for_either_later == 0)


-- The computer does not step its last man where the person's can jump it, and looks ahead until its checks run out.
test the_computer_keeps_its_man_out_of_reach
        covers computer_move > deepening'many'1lt > candidate'many > finished,
               computer_move > deepening'many'1lt > candidate'many > ran_out, searched > judged.leaf'2,
               searched > judged.spent'gt, searched > judged.inner > candidate > others_turn,
               searched > judged.inner > candidate'many > owners_turn,
               searched > judged.inner > candidate'many > others_turn,
               best_sampled > option'many > ranked.top > ties.settled,
               best_sampled > option'many > ranked.lower > ties.settled:
    inject:
            random = fake random(seed: 7)
    given:
        mutable game = arranged([man(side.white, 2, 3), man(side.red, 4, 5)], to_move: side.white)
        game.level := levels.all[1]
    when:
        chosen = computer_move(game)
    then:
        out_of_reach = (chosen.path == [square(row: 3, column: 2)])


-- When the minimax search weighs several moves the same, the games played out at random from each choose among them.
test the_computer_plays_out_games_to_choose_an_opening
        covers apply > captured'many, chains > direction'many > cannot_jump'2, chains_after > uncrowned'1,
               best_sampled > option'many > ranked.lower > ties.contested > option'many:
    inject:
            random = fake random(seed: 7)
    given:
        mutable game = new_game(levels.all[1])
        opening = legal_moves(game)[0]
        apply(mutable game, opening)
    when:
        chosen = computer_move(game)
    then:
        a_white_move = legal_moves(game).contains(chosen)


-- With no checks to play games out, a position is worth its pieces.
test without_checks_the_pieces_decide
        covers sampled_value > no_share, material > placed'many > counted.king, material > placed'many > counted.man,
               material > placed'many > counted.other:
    inject:
            random = fake random(seed: 7)
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 1, 2)], to_move: side.red)
    when:
        worth = sampled_value(game, side.red, 0)
    then:
        the_pieces = (worth == 2)


-- A click on the board finds the square under it, and a click off it finds none.
test a_click_finds_its_square
        covers square_at > inside, square_at > outside'1, square_at > outside'2:
    when:
        on = square_at(point(x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5))
        off = square_at(point(x: 5, y: 5))
        below = square_at(point(x: 100, y: 700))
    then:
        found = (on == optional.some(square(row: 5, column: 3)))
        none = (off == optional.none)
        none_below = (below == optional.none)


-- A click on a level's button finds the level, and a click elsewhere finds none.
test a_click_finds_its_level
        covers level_at > level'many > pressed, level_at > level'many > missed'2:
    when:
        on = level_at(point(x: 100, y: layout.button_top + layout.button_step + 10))
        off = level_at(point(x: 100, y: 10))
    then:
        casual = (on == optional.some(levels.all[1]))
        none = (off == optional.none)


-- The squares are light and dark, and marked when they hold the selected piece or were part of the last move.
test squares_are_shaded
        covers shade_of > looks.light, shade_of > looks.selected, shade_of > looks.last, shade_of > looks.dark,
               in_last_move > made.some:
    given:
        fresh = new_game(levels.all[0])
        mutable moved = new_game(levels.all[0])
        opening = legal_moves(moved)[0]
        apply(mutable moved, opening)
        from = moved.last.from
        selected = optional.some(square(row: 2, column: 1))
    when:
        light = shade_of(fresh, square(row: 0, column: 0), selected)
        chosen = shade_of(fresh, square(row: 2, column: 1), selected)
        plain = shade_of(fresh, from, selected)
        last = shade_of(moved, from, selected)
    then:
        light_square = (light == palette.light)
        selected_square = (chosen == palette.selected)
        dark_square = (plain == palette.dark)
        last_move = (last == palette.last)


-- The selected piece shows where it may move, and with none selected nothing is shown.
test the_selected_piece_shows_where_it_may_go
        covers landings > candidate'many > from_selected, landings > candidate'many > from_elsewhere:
    given:
        game = new_game(levels.all[0])
    when:
        some = landings(game, optional.some(square(row: 5, column: 2)))
        none = landings(game, optional.none)
    then:
        two_squares = (some == [square(row: 4, column: 1), square(row: 4, column: 3)])
        nothing = none.is_empty()


-- Pieces are drawn in their player's colours, with a crown for a king.
test pieces_are_painted_by_player_and_rank
        covers colours_of > painted.red_king, colours_of > painted.red_man, colours_of > painted.white_king,
               colours_of > painted.white_man:
    when:
        red_king = colours_of(piece(owner: side.red, king: true))
        red_man = colours_of(piece(owner: side.red, king: false))
        white_king = colours_of(piece(owner: side.white, king: true))
        white_man = colours_of(piece(owner: side.white, king: false))
    then:
        crowned_red = (red_king == [palette.red, palette.red_ring, palette.crown])
        plain_red = (red_man == [palette.red, palette.red_ring, palette.red])
        crowned_white = (white_king == [palette.white, palette.white_ring, palette.crown])
        plain_white = (white_man == [palette.white, palette.white_ring, palette.white])


-- The banner says whose move it is, and the score once the game has ended.
test the_banner_says_how_the_game_stands
        covers banner > shown.your_move, banner > shown.thinking, banner > shown.won, banner > shown.lost,
               banner > shown.drawn:
    given:
        fresh = new_game(levels.all[2])
        mutable thinking = new_game(levels.all[2])
        opening = legal_moves(thinking)[0]
        apply(mutable thinking, opening)
        mutable won = arranged([man(side.red, 4, 3)], to_move: side.white)
        won.turns := 59
        lost = arranged([man(side.white, 3, 2)], to_move: side.red)
        mutable drawn = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        drawn.quiet := game_rules.quiet_limit
    when:
        fresh_lines = banner(fresh, 50)
        thinking_lines = banner(thinking, 50)
        won_lines = banner(won, 2100)
        lost_lines = banner(lost, 50)
        drawn_lines = banner(drawn, 50)
    then:
        says_your_move = (fresh_lines[0] == "Your move   Club   Moves 0   Best 50")
        says_thinking = (thinking_lines[0] == "The computer is thinking, at 2500 positions a move...")
        says_won = (won_lines[0] == "You won in 30 moves against Club! Score 2100   Best 2100")
        says_lost = (lost_lines[0] == "The computer won. Score 0   Best 50")
        says_drawn = (drawn_lines[0] == "A draw: 80 turns without a capture. Score 0   Best 50")


-- A click on neither a piece that may move nor a square the selected piece may move to selects nothing.
test a_click_elsewhere_selects_nothing
        covers centre_of, clicked > judged.elsewhere, move_to > candidate'many > differs'1,
               move_to > candidate'many > differs'2, may_move_from > candidate'many > unmovable:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.some(square(row: 5, column: 2))
    when:
        clicked(mutable game, mutable selected, centre_of(square(row: 0, column: 0)))
    then:
        nothing_selected = (selected == optional.none)
        red_still_to_move = (game.to_move == side.red)


-- Escape on the levels ends play before a game.
test escape_at_the_levels_ends_play
        covers draw_levels > level'many, choose_level > choosing, level_clicked > no_clicks:
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
        covers playout > ended'1, draw_marks > landing'many, play > open > picked.level, choose_level > choosing'many,
               play_game > playing'many > moment.computers_turn, play_game > playing'many > moment.players_turn,
               level_clicked > place > miss, act_on_clicks > place:
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


-- Once a game has ended, Space ends it with its score, to choose another.
test space_after_a_game_chooses_another
        covers draw_pieces > placed, draw_piece, play_game > playing > moment.over:
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


--------------------------------------------------------------------------------------------------------------------------------
A level that looks at a given number of positions a move, for the tests of the search.

checks: How many positions, as uint32.

made: The level, as level.
--------------------------------------------------------------------------------------------------------------------------------
function trial(checks) returns made:
    return made = level(name: "Trial", checks: checks, weight: 1)


-- A man with an edge on one side and a piece he cannot jump on the other has no step, and loses.
test a_blocked_man_has_no_step
        covers all_steps > placed > direction'many > blocked'1, all_steps > placed > direction'many > blocked'2,
               jumpable > outside, directions_of > moving.red:
    given:
        game = arranged([man(side.red, 1, 0), man(side.white, 0, 1)], to_move: side.red)
    when:
        moves = legal_moves(game)
    then:
        none = moves.is_empty()
        lost = (standing(game) == outcome.lost)


-- A board with no pieces has none to move, draw or weigh.
test an_empty_board_has_nothing_on_it
        covers own_pieces > no_pieces, material > no_pieces, draw, corner_of, in_last_move > made.none,
               landings > no_moves, draw_board > row'many > column'many, draw_pieces > no_pieces, draw_marks > no_marks,
               move_to > no_moves, may_move_from > no_moves:
    inject:
            window = fake window(frames: [])
    given:
        game = arranged([], to_move: side.red)
    when:
        pieces = own_pieces(game)
        worth = material(game, side.red)
        found = move_to(game, optional.some(square(row: 5, column: 0)), optional.some(square(row: 4, column: 1)))
        movable = may_move_from(game, optional.some(square(row: 5, column: 0)))
        marks = landings(game, optional.some(square(row: 5, column: 0)))
        draw(game, optional.none, 0)
    then:
        no_pieces = pieces.is_empty()
        no_worth = (worth == 0)
        no_move = (found == optional.none)
        not_movable = (not movable)
        no_marks = marks.is_empty()
        banner_written = window.written().contains("The computer won. Score 0   Best 0")


-- With one move in the game, it is found from its square to its square, and from nowhere else.
test the_only_move_is_found
        covers own_pieces > placed'many > mine, own_pieces > placed'many > theirs, landings > candidate > from_selected,
               draw_marks > landing, move_to > candidate > fits, move_to > candidate > differs'1,
               move_to > candidate > differs'2, may_move_from > candidate > movable,
               may_move_from > candidate > unmovable:
    inject:
            window = fake window(frames: [])
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
        from = optional.some(square(row: 5, column: 0))
    when:
        fits = move_to(game, from, optional.some(square(row: 4, column: 1)))
        elsewhere = move_to(game, optional.some(square(row: 0, column: 0)), optional.some(square(row: 4, column: 1)))
        too_far = move_to(game, from, optional.some(square(row: 3, column: 2)))
        movable = may_move_from(game, from)
        unmovable = may_move_from(game, optional.some(square(row: 6, column: 1)))
        marks = landings(game, from)
        pieces = own_pieces(game)
        draw(game, from, 0)
    then:
        found = (fits != optional.none)
        not_from_there = (elsewhere == optional.none)
        not_to_there = (too_far == optional.none)
        from_its_square = movable
        not_from_another = (not unmovable)
        one_mark = (marks == [square(row: 4, column: 1)])
        one_of_red = (pieces.count() == 1)


-- A lone piece of each kind is worth its own value to its player, and nothing to the other.
test a_lone_piece_is_worth_its_rank
        covers own_pieces > placed > mine, piece_at, number_of, material > placed > counted.king,
               material > placed > counted.man, material > placed > counted.other:
    given:
        lone_king = arranged([king(side.red, 4, 3)], to_move: side.red)
        lone_man = arranged([man(side.red, 4, 3)], to_move: side.red)
    when:
        king_for_red = material(lone_king, side.red)
        king_for_white = material(lone_king, side.white)
        man_for_red = material(lone_man, side.red)
        pieces = own_pieces(lone_man)
    then:
        a_king = (king_for_red == search.king_value)
        nothing = (king_for_white == 0)
        a_man = (man_for_red == search.man_value)
        one_piece = (pieces.count() == 1)


-- With one move to choose from, it is chosen without weighing it.
test one_option_is_chosen_unweighed
        covers best_sampled > option > ranked.top > ties.settled:
    inject:
            random = fake random(seed: 7)
    given:
        game = arranged([man(side.red, 5, 0), man(side.white, 0, 7)], to_move: side.red)
        options = legal_moves(game)
    when:
        chosen = best_sampled(game, options, [0], 10)
    then:
        the_only_one = (chosen == options[0])


-- With almost no checks, the search looks one move ahead and runs out at once.
test with_no_checks_the_search_runs_out_at_once
        covers computer_move > deepening > candidate > ran_out, computer_move > deepening > candidate'many > ran_out,
               searched > judged.leaf'1, best_sampled > option'many > ranked.top > ties.contested > option'many:
    inject:
            random = fake random(seed: 7)
    given:
        mutable one = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        one.level := trial(1)
        mutable two = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        two.level := trial(1)
    when:
        only = computer_move(one)
        either = computer_move(two)
    then:
        the_only_move = (only.path == [square(row: 3, column: 6)])
        a_white_move = legal_moves(two).contains(either)


-- With a few checks, the search finishes a few depths, and runs out partway.
test with_few_checks_the_search_runs_out_partway
        covers chains_after > uncrowned'2lt, computer_move > deepening'many'1lt > candidate > finished,
               computer_move > deepening'many'1lt > candidate > ran_out, sampled_value > some_share > sampling,
               playout > going'many:
    inject:
            random = fake random(seed: 7)
    given:
        mutable one = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        one.level := trial(24)
        mutable two = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        two.level := trial(46)
    when:
        only = computer_move(one)
        either = computer_move(two)
    then:
        the_only_move = (only.path == [square(row: 3, column: 6)])
        a_white_move = legal_moves(two).contains(either)


-- With enough checks, the search reaches its deepest, and runs out there.
test the_search_runs_out_at_its_deepest
        covers computer_move > deepening'many'1eq > candidate > ran_out,
               computer_move > deepening'many'1eq > candidate'many > ran_out:
    inject:
            random = fake random(seed: 7)
    given:
        mutable one = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        one.level := trial(800)
        mutable two = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        two.level := trial(1500)
    when:
        only = computer_move(one)
        either = computer_move(two)
    then:
        the_only_move = (only.path == [square(row: 3, column: 6)])
        a_white_move = legal_moves(two).contains(either)


-- With more checks still, the search finishes its deepest, and stops there.
test the_search_finishes_its_deepest
        covers computer_move > deepening'many'1eq > candidate > finished,
               computer_move > deepening'many'1eq > candidate'many > finished,
               searched > judged.inner > candidate > owners_turn, sampled_value > some_share > sampling'many:
    inject:
            random = fake random(seed: 7)
    given:
        mutable one = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
        one.level := trial(1100)
        mutable two = arranged([man(side.white, 2, 5), man(side.red, 7, 0)], to_move: side.white)
        two.level := trial(2100)
    when:
        only = computer_move(one)
        either = computer_move(two)
    then:
        the_only_move = (only.path == [square(row: 3, column: 6)])
        a_white_move = legal_moves(two).contains(either)


-- The search stops at a drawn position, and gives up when it has exactly the checks a position's moves need.
test the_search_stops_at_a_draw_and_when_spent
        covers searched > judged.leaf'3eq, searched > judged.leaf'3gt, searched > judged.spent'eq:
    given:
        mutable drawn = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        drawn.quiet := game_rules.quiet_limit
        mutable later = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        later.quiet := game_rules.quiet_limit + 1
        open = arranged([man(side.red, 5, 2), man(side.white, 2, 7)], to_move: side.red)
        mutable plenty = 100
        mutable exact = 2
    when:
        at_draw = searched(drawn, side.red, 3, mutable plenty)
        after_draw = searched(later, side.red, 3, mutable plenty)
        spent = searched(open, side.red, 3, mutable exact)
    then:
        nothing_for_a_draw = ((at_draw == 0) and (after_draw == 0))
        none_left = (exact == 0)
        given_up = (spent == 0)


-- A game played out ends at once when it has no moves or is drawn, or after the one move it may make.
test a_game_played_out_may_end_at_once
        covers playout > ended'2, playout > ended'3eq, playout > ended'3gt, playout > going:
    inject:
            random = fake random(seed: 7)
    given:
        stuck = arranged([man(side.red, 1, 0), man(side.white, 0, 1)], to_move: side.red)
        mutable drawn = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        drawn.quiet := game_rules.quiet_limit
        mutable later = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        later.quiet := game_rules.quiet_limit + 1
        open = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
    when:
        no_moves = playout(stuck, side.red, 5)
        at_draw = playout(drawn, side.red, 5)
        after_draw = playout(later, side.red, 5)
        one_move = playout(open, side.red, 1)
    then:
        lost_at_once = (no_moves == played_out(value: -search.win_value, spent: 1))
        drawn_at_once = ((at_draw.spent == 1) and (after_draw.spent == 1))
        two_positions = (one_move.spent == 2)


-- A click beside the buttons chooses no level; of two clicks in a frame the first on a button counts, and two that
-- miss choose nothing.
test clicks_beside_and_between_the_buttons
        covers level_at > level > pressed, level_at > level'many > missed'1, level_clicked > place'many > hit,
               level_clicked > place'many > miss:
    when:
        beside = level_at(point(x: 595, y: layout.button_top + 10))
        second = level_clicked([point(x: 100, y: 10), point(x: 100, y: layout.button_top + 10)])
        neither = level_clicked([point(x: 100, y: 10), point(x: 595, y: 10)])
    then:
        none_beside = (beside == optional.none)
        the_second = (second == optional.some(levels.all[0]))
        none = (neither == optional.none)


-- Two clicks in one frame select a man, then move him.
test two_clicks_in_a_frame_make_a_move
        covers clicked > judged.moving, clicked > judged.choosing, act_on_clicks > place'many,
               move_to > candidate'many > fits, may_move_from > candidate'many > movable:
    given:
        mutable game = new_game(levels.all[0])
        mutable selected = optional.none
    when:
        act_on_clicks(mutable game, mutable selected, [
            centre_of(square(row: 5, column: 2)),
            centre_of(square(row: 4, column: 3)),
        ])
    then:
        moved = (piece_at(game, square(row: 4, column: 3)) == optional.some(piece(owner: side.red, king: false)))
        white_to_move = (game.to_move == side.white)


-- A window closed at once ends play without a game.
test a_window_closed_at_once_ends_play
        covers play > open > picked.none, choose_level > closed_at_once'3:
    inject:
            window = fake window(frames: [])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)


-- A window closed as a game starts ends that game, and then play.
test a_window_closed_as_a_game_starts_ends_play
        covers play > open'many > picked.none, play > open'many > picked.level, play_game > closed_at_once'3,
               level_clicked > place > hit:
    inject:
            window = fake window(frames: [frame(clicks: [point(x: 100, y: layout.button_top + 10)])])
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_score = (best == 0)


-- The computer moves in a frame of its own.
test the_computer_moves_in_a_frame
        covers play_game > playing > moment.computers_turn:
    inject:
            window = fake window(frames: [frame()])
            random = fake random(seed: 7)
            forever = fake forever
    given:
        computers = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.white)
    when:
        after_computer = play_game(computers, 0)
    then:
        not_quitting = (after_computer == ending(points: 0, quitting: false))


-- The player acts in a frame of their own.
test the_player_moves_in_a_frame
        covers landings > candidate > from_elsewhere, draw_pieces > placed'many,
               play_game > playing > moment.players_turn, act_on_clicks > no_clicks:
    inject:
            window = fake window(frames: [frame()])
            random = fake random(seed: 7)
            forever = fake forever
    given:
        players = arranged([man(side.white, 2, 7), man(side.red, 7, 0)], to_move: side.red)
    when:
        ended = play_game(players, 0)
    then:
        not_quitting = (ended == ending(points: 0, quitting: false))


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

