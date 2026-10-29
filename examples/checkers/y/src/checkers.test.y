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
        covers starting_piece#placed.light, starting_piece#placed.white_men, starting_piece#placed.red_men,
               starting_piece#placed.between, placed_pieces#held.empty, placed_pieces#held.piece, standing#judged.playing:
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
test the_first_moves_are_steps
        covers legal_moves#no_jump, all_steps#open, all_steps#blocked, own_pieces#mine, own_pieces#theirs,
               directions_of#moving.red, directions_of#moving.white, chains#cannot_jump, jumpable#inside, jumpable#outside,
               belongs_to#held.empty, belongs_to#held.piece, apply#quiet, other#current.red:
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
        covers legal_moves#must_jump, chains#can_jump, chains#ends_here, chains_after#uncrowned, apply#capturing:
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
        covers chains#goes_on:
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
        covers chains_after#crowned, far_row#heading_up:
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
        covers directions_of#moving.king:
    given:
        game = arranged([king(side.red, 4, 3), man(side.white, 0, 1)], to_move: side.red)
    when:
        moves = legal_moves(game)
    then:
        four_ways = (moves.count() == 4)


-- White's men are crowned on the bottom row.
test white_is_crowned_on_the_bottom_row
        covers far_row#heading_down, other#current.white:
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
        covers standing#judged.won, score#won:
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
        covers standing#judged.lost, score#unfinished:
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
        covers standing#judged.drawn:
    given:
        mutable game = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        game.quiet := game_rules.quiet_limit
    when:
        reached = standing(game)
    then:
        drawn = (reached == outcome.drawn)


-- A position is worth the pieces of each side, kings more than men.
test pieces_are_worth_their_rank
        covers value_for#judged.playing, material#counted.other, material#counted.king, material#counted.man:
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
        covers value_for#judged.losing, value_for#judged.winning, value_for#judged.drawn:
    given:
        stuck = arranged([man(side.red, 4, 3)], to_move: side.white)
        mutable drawn = arranged([man(side.red, 5, 0), man(side.white, 2, 7)], to_move: side.red)
        drawn.quiet := game_rules.quiet_limit
    when:
        for_white = value_for(stuck, side.white)
        for_red = value_for(stuck, side.red)
        for_either = value_for(drawn, side.red)
    then:
        white_lost = (for_white == -search.win_value)
        red_won = (for_red == search.win_value)
        nothing_for_a_draw = (for_either == 0)


-- The computer does not step its last man where the person's can jump it, and looks ahead until its checks run out.
test the_computer_keeps_its_man_out_of_reach
        covers computer_move#deepening, computer_move#finished, computer_move#ran_out, computer_move#deep_enough,
               searched#judged.leaf, searched#judged.spent, searched#judged.inner, searched#owners_turn,
               searched#others_turn, best_sampled#top, best_sampled#lower, best_sampled#settled:
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
        covers best_sampled#contested, sampled_value#sampling, sampled_value#sampled, sampled_value#any, playout#going,
               playout#ended:
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
        covers sampled_value#none:
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
        covers square_at#inside, square_at#outside:
    when:
        on = square_at(point(x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5))
        off = square_at(point(x: 5, y: 5))
    then:
        found = (on == optional.some(square(row: 5, column: 3)))
        none = (off == optional.none)


-- A click on a level's button finds the level, and a click elsewhere finds none.
test a_click_finds_its_level
        covers level_at#pressed, level_at#missed:
    when:
        on = level_at(point(x: 100, y: layout.button_top + layout.button_step + 10))
        off = level_at(point(x: 100, y: 10))
    then:
        casual = (on == optional.some(levels.all[1]))
        none = (off == optional.none)


-- The squares are light and dark, and marked when they hold the selected piece or were part of the last move.
test squares_are_shaded
        covers shade_of#looks.light, shade_of#looks.selected, shade_of#looks.last, shade_of#looks.dark,
               in_last_move#made.none, in_last_move#made.some:
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
        covers landings#from_selected, landings#from_elsewhere:
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
        covers colours_of#painted.red_king, colours_of#painted.red_man, colours_of#painted.white_king,
               colours_of#painted.white_man:
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
        covers banner#shown.your_move, banner#shown.thinking, banner#shown.won, banner#shown.lost, banner#shown.drawn:
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
        covers clicked#judged.elsewhere:
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
        covers play#open, play#picked.none, play#closed, choose_level#choosing, choose_level#chosen_or_stopped:
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
        covers choose_level#miss, choose_level#hit, play#picked.level, play_game#playing, play_game#stopped,
               play_game#moment.players_turn, play_game#moment.computers_turn, clicked#judged.choosing,
               clicked#judged.moving, move_to#fits, move_to#differs, may_move_from#movable, may_move_from#unmovable:
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
        covers play_game#moment.over:
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
