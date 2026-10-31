------------------------------------------------------------------------------------------------------------------------------------------------
A game with a snake one cell long in the middle of the board, moving one way, and the food in the top left corner.

heading: The way the snake moves, as direction.

started: The game, as game.
------------------------------------------------------------------------------------------------------------------------------------------------
function moving(heading) returns started:
    return started = game(
        head: cell(column: 10, row: 7),
        tail: [],
        heading: heading,
        wanted: heading,
        food: cell(column: 0, row: 0),
        eaten: 0,
        standing: outcome.playing,
    )


-- A new game has a snake three cells long, moving right, and food in a cell the snake is not in.
test a_new_game_puts_the_food_in_a_free_cell
        covers new_game:
    inject:
            random = fake random(seed: 7)
    when:
        started = new_game()
    then:
        three_long = (started.tail.count() == 2)
        moving_right = (started.heading == direction.right)
        food_off_the_snake = ((started.food != started.head) and (not started.tail.contains(started.food)))


-- The up arrow turns the snake up.
test the_up_arrow_turns_the_snake_up
        covers turned > arrow.up'1:
    given:
        game = moving(direction.right)
    when:
        wanted = turned(game, key.up)
    then:
        turned_up = (wanted == direction.up)


-- W turns the snake up.
test w_turns_the_snake_up
        covers turned > arrow.up'2:
    given:
        game = moving(direction.right)
    when:
        wanted = turned(game, key.w)
    then:
        turned_up = (wanted == direction.up)


-- The down arrow turns the snake down.
test the_down_arrow_turns_the_snake_down
        covers turned > arrow.down'1:
    given:
        game = moving(direction.right)
    when:
        wanted = turned(game, key.down)
    then:
        turned_down = (wanted == direction.down)


-- S turns the snake down.
test s_turns_the_snake_down
        covers turned > arrow.down'2:
    given:
        game = moving(direction.right)
    when:
        wanted = turned(game, key.s)
    then:
        turned_down = (wanted == direction.down)


-- The left arrow turns the snake left.
test the_left_arrow_turns_the_snake_left
        covers turned > arrow.left'1:
    given:
        game = moving(direction.up)
    when:
        wanted = turned(game, key.left)
    then:
        turned_left = (wanted == direction.left)


-- A turns the snake left.
test a_turns_the_snake_left
        covers turned > arrow.left'2:
    given:
        game = moving(direction.up)
    when:
        wanted = turned(game, key.a)
    then:
        turned_left = (wanted == direction.left)


-- The right arrow turns the snake right.
test the_right_arrow_turns_the_snake_right
        covers turned > arrow.right'1:
    given:
        game = moving(direction.up)
    when:
        wanted = turned(game, key.right)
    then:
        turned_right = (wanted == direction.right)


-- D turns the snake right.
test d_turns_the_snake_right
        covers turned > arrow.right'2:
    given:
        game = moving(direction.up)
    when:
        wanted = turned(game, key.d)
    then:
        turned_right = (wanted == direction.right)


-- A key that is not an arrow, nor one of W, A, S and D, leaves the snake going as the player last steered it.
test a_key_that_is_not_an_arrow_does_not_turn_the_snake
        covers turned > arrow.other:
    given:
        mutable game = moving(direction.right)
        game.wanted := direction.up
    when:
        wanted = turned(game, key.space)
    then:
        unchanged = (wanted == direction.up)


-- The tail follows the head, and the snake keeps its length.
test the_tail_follows_the_head
        covers advance > moved.on:
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7), cell(column: 8, row: 7)]
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        moved = (game.head == cell(column: 11, row: 7))
        followed = (game.tail == [cell(column: 10, row: 7), cell(column: 9, row: 7)])


-- The head may move into the cell the end of the tail is leaving.
test the_snake_may_follow_its_own_tail
        covers advance > moved.on:
    given:
        mutable game = moving(direction.left)
        game.head := cell(column: 5, row: 5)
        game.tail := [cell(column: 6, row: 5), cell(column: 6, row: 6), cell(column: 5, row: 6)]
        game.wanted := direction.down
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        still_playing = (game.standing == outcome.playing)
        moved = (game.head == cell(column: 5, row: 6))


-- Moving into the wall ends the game, and leaves the snake where it was.
test the_wall_ends_the_game
        covers advance > moved.into_wall:
    given:
        mutable game = moving(direction.right)
        game.head := cell(column: 19, row: 7)
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        crashed = (game.standing == outcome.crashed)
        not_moved = (game.head == cell(column: 19, row: 7))


-- Moving into the snake's own body ends the game.
test the_snake_cannot_bite_itself
        covers advance > moved.into_itself:
    given:
        -- Moving left along the top of a loop, then steered down into it.
        mutable game = moving(direction.left)
        game.head := cell(column: 5, row: 5)
        game.tail := [cell(column: 6, row: 5), cell(column: 6, row: 6), cell(column: 5, row: 6), cell(column: 4, row: 6)]
        game.wanted := direction.down
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        bitten = (game.standing == outcome.bitten)
        not_moved = (game.head == cell(column: 5, row: 5))


-- Moving onto the food makes the snake one cell longer, scores one, and puts new food in a free cell.
test eating_grows_the_snake
        covers advance > moved.onto_food:
    given:
        mutable game = moving(direction.left)
        game.food := cell(column: 9, row: 7)
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        grown = (game.tail == [cell(column: 10, row: 7)])
        scored = (game.eaten == 1)
        new_food = ((game.food != game.head) and (not game.tail.contains(game.food)))


-- When the snake eats the last food there is room for, it fills the board, and the game ends.
test eating_the_last_food_fills_the_board
        covers advance > moved.onto_food:
    given:
        corner = cell(column: 0, row: 0)
        mutable game = moving(direction.left)
        game.head := cell(column: 1, row: 0)
        game.tail := [corner]

        -- Every cell but the corner and the one beside it.
        rest = free_cells(game)
        game.tail := rest
        game.food := corner
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        scored = (game.eaten == 1)
        filled = (game.standing == outcome.filled)


-- Steering back the way the snake moved last leaves it going as the player steered it before.
test steering_back_the_way_the_snake_moved_keeps_the_way_wanted
        covers unless_reversing > reversing:
    given:
        mutable game = moving(direction.right)
        game.wanted := direction.up
    when:
        wanted = unless_reversing(game, direction.left)
    then:
        still_up = (wanted == direction.up)


-- Steering any way but back turns the snake that way.
test steering_another_way_turns_the_snake
        covers unless_reversing > turning:
    given:
        game = moving(direction.right)
    when:
        wanted = unless_reversing(game, direction.up)
    then:
        turned_up = (wanted == direction.up)


-- The way back from up is down.
test the_opposite_of_up_is_down
        covers opposite > going.up:
    when:
        back = opposite(direction.up)
    then:
        down = (back == direction.down)


-- The way back from down is up.
test the_opposite_of_down_is_up
        covers opposite > going.down:
    when:
        back = opposite(direction.down)
    then:
        up = (back == direction.up)


-- The way back from left is right.
test the_opposite_of_left_is_right
        covers opposite > going.left:
    when:
        back = opposite(direction.left)
    then:
        right = (back == direction.right)


-- The way back from right is left.
test the_opposite_of_right_is_left
        covers opposite > going.right:
    when:
        back = opposite(direction.right)
    then:
        left = (back == direction.left)


-- The cell ahead going up is the next one up.
test the_cell_ahead_going_up
        covers ahead_of > going.up:
    when:
        ahead = ahead_of(cell(column: 10, row: 7), direction.up)
    then:
        next_up = (ahead == cell(column: 10, row: 6))


-- The cell ahead going down is the next one down.
test the_cell_ahead_going_down
        covers ahead_of > going.down:
    when:
        ahead = ahead_of(cell(column: 10, row: 7), direction.down)
    then:
        next_down = (ahead == cell(column: 10, row: 8))


-- The cell ahead going left is the next one left.
test the_cell_ahead_going_left
        covers ahead_of > going.left:
    when:
        ahead = ahead_of(cell(column: 10, row: 7), direction.left)
    then:
        next_left = (ahead == cell(column: 9, row: 7))


-- The cell ahead going right is the next one right.
test the_cell_ahead_going_right
        covers ahead_of > going.right:
    when:
        ahead = ahead_of(cell(column: 10, row: 7), direction.right)
    then:
        next_right = (ahead == cell(column: 11, row: 7))


-- The bottom right corner of the board is on it.
test the_far_corner_is_on_the_board
        covers on_board:
    when:
        inside = on_board(cell(column: 19, row: 14))
    then:
        on = inside


-- A cell just past the right edge of the board is off it.
test a_cell_past_the_edge_is_off_the_board
        covers on_board:
    when:
        inside = on_board(cell(column: 20, row: 7))
    then:
        off = (not inside)


-- Every cell the snake is not in is free, from the top left.
test cells_the_snake_is_not_in_are_free
        covers free_cells > row'many > column'many > vacant:
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7)]
    when:
        free = free_cells(game)
    then:
        all_the_others = (free.count() == 298)
        from_the_top_left = (free.first() == optional.some(cell(column: 0, row: 0)))


-- The cell the head is in is not free.
test the_cell_of_the_head_is_not_free
        covers free_cells > row'many > column'many > taken'1:
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7)]
    when:
        free = free_cells(game)
    then:
        head_taken = (not free.contains(game.head))


-- The cells the tail is in are not free.
test the_cells_of_the_tail_are_not_free
        covers free_cells > row'many > column'many > taken'2:
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7)]
    when:
        free = free_cells(game)
    then:
        tail_taken = (not free.contains(cell(column: 9, row: 7)))


-- With no free cell left for the food, the snake fills the board, and the game ends.
test food_with_no_free_cell_fills_the_board
        covers place_food > full:
    given:
        mutable game = moving(direction.left)
        game.head := cell(column: 0, row: 0)
        game.tail := free_cells(game)
    inject:
            random = fake random(seed: 7)
    when:
        place_food(mutable game)
    then:
        filled = (game.standing == outcome.filled)


-- The food goes in a free cell: here the only one, the bottom right corner.
test food_goes_in_a_free_cell
        covers place_food > room:
    given:
        mutable game = moving(direction.left)
        game.head := cell(column: 0, row: 0)
        game.tail := free_cells(game)
        game.tail.remove_last()
    inject:
            random = fake random(seed: 7)
    when:
        place_food(mutable game)
    then:
        in_the_free_cell = (game.food == cell(column: 19, row: 14))
        still_playing = (game.standing == outcome.playing)


-- While a game is played, the banner shows the score, the best score, and the keys.
test the_banner_of_a_game_in_play_shows_the_score_and_the_best
        covers banner > shown.playing:
    given:
        mutable game = moving(direction.right)
        game.eaten := 3
    when:
        line = banner(game, 5)
    then:
        told = (line == "Score 3   Best 5   Arrows steer, Escape quits")


-- Once the snake crashes into the wall, the banner says so, with the score.
test the_banner_tells_of_a_crash_into_the_wall
        covers banner > shown.crashed:
    given:
        mutable game = moving(direction.right)
        game.standing := outcome.crashed
        game.eaten := 2
    when:
        line = banner(game, 5)
    then:
        told = (line == "Crashed into the wall with 2. Space plays again.")


-- Once the snake bites itself, the banner says so, with the score.
test the_banner_tells_of_a_bite
        covers banner > shown.bitten:
    given:
        mutable game = moving(direction.right)
        game.standing := outcome.bitten
        game.eaten := 4
    when:
        line = banner(game, 5)
    then:
        told = (line == "Bit itself with 4. Space plays again.")


-- Once the snake fills the board, the banner says so, with the score.
test the_banner_tells_of_a_filled_board
        covers banner > shown.filled:
    given:
        mutable game = moving(direction.right)
        game.standing := outcome.filled
        game.eaten := 298
    when:
        line = banner(game, 5)
    then:
        told = (line == "Filled the board with 298! Space plays again.")


-- A snake of one cell, with no tail, is drawn, with the banner beneath.
test a_snake_with_no_tail_is_drawn
        covers draw > no_tail:
    inject:
            window = fake window(frames: [])
    given:
        game = moving(direction.right)
    when:
        draw(game, 2)
    then:
        banner_written = (window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"])


-- A snake with a tail of one cell is drawn, with the banner beneath.
test a_snake_with_a_tail_of_one_cell_is_drawn
        covers draw > part:
    inject:
            window = fake window(frames: [])
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7)]
    when:
        draw(game, 2)
    then:
        banner_written = (window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"])


-- A snake with a tail of several cells is drawn, with the banner beneath.
test a_snake_with_a_longer_tail_is_drawn
        covers draw > part'many:
    inject:
            window = fake window(frames: [])
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7), cell(column: 8, row: 7)]
    when:
        draw(game, 2)
    then:
        banner_written = (window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"])


-- Filling a cell fills one square, and writes no text.
test filling_a_cell_fills_one_square
        covers fill_cell:
    inject:
            window = fake window(frames: [])
    when:
        fill_cell(cell(column: 3, row: 4), palette.food)
    then:
        one_square = (window.fills() == 1)
        nothing_written = window.written().is_empty()


-- A window closed before the first frame ends play, with no score.
test a_window_closed_at_once_ends_play
        covers play > closed_at_once'2:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 150ms)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        no_frames = (window.frames_shown() == 0)
        no_score = (best == 0)


-- Escape ends play at the end of the frame it is pressed in.
test escape_quits
        covers play > open > going_on'1:
    inject:
            window = fake window(frames: [[key.escape], [], []])
            clock = fake clock(tick: 150ms)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        one_frame = (window.frames_shown() == 1)
        no_score = (best == 0)


-- The game is drawn in each frame, and the snake steps once for each step time that passes, until the window closes.
test the_game_plays_until_the_window_closes
        covers play > open'many > going_on'1:
    inject:
            window = fake window(frames: [[], [], []])
            clock = fake clock(tick: 150ms)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        three_frames = (window.frames_shown() == 3)
        drawn = (window.written().last() == optional.some("Score 0   Best 0   Arrows steer, Escape quits"))
        no_score = (best == 0)


-- Once a game ends, Space starts the next.
test space_starts_the_next_game
        covers play > open'many > again:
    inject:
            -- Fourteen steps right take the snake from column 6 into the wall. The game waits for a frame with no key, and
            -- Space then starts it again.
            window = fake window(frames: [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [key.space], []])
            clock = fake clock(tick: 150ms)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        crashed = window.written().contains("Crashed into the wall with 0. Space plays again.")
        started_again = (window.written().last() == optional.some("Score 0   Best 0   Arrows steer, Escape quits"))
        no_score = (best == 0)


-- Once a game ends, it stays over, frame after frame, until the player presses Space.
test a_finished_game_waits_for_space
        covers play > open'many > going_on'2:
    inject:
            -- Fourteen steps right take the snake from column 6 into the wall, and no frame after presses Space.
            window = fake window(frames: [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []])
            clock = fake clock(tick: 150ms)
            random = fake random(seed: 7)
            forever = fake forever
    when:
        best = play()
    then:
        still_over = (window.written().last() == optional.some("Crashed into the wall with 0. Space plays again."))
        no_score = (best == 0)


-- A frame with no key pressed leaves the snake going as the player last steered it.
test no_key_leaves_the_way_wanted
        covers steer > no_keys:
    given:
        mutable game = moving(direction.right)
    when:
        steer(mutable game, [])
    then:
        unchanged = (game.wanted == direction.right)


-- A key pressed in a frame steers the snake the way it points.
test a_key_pressed_steers_the_snake
        covers steer > pressed:
    given:
        mutable game = moving(direction.right)
    when:
        steer(mutable game, [key.up])
    then:
        up = (game.wanted == direction.up)


-- Two keys pressed in one frame steer in the order they were pressed.
test two_keys_in_a_frame_steer_in_turn
        covers steer > pressed'many:
    given:
        mutable game = moving(direction.right)
    when:
        steer(mutable game, [key.up, key.left])
    then:
        -- Expected up to stand: left, after up, is back the way the snake moved last.
        up = (game.wanted == direction.up)


-- A game that is over does not step, however much time passes.
test a_game_that_is_over_does_not_step
        covers catch_up > not_due'1:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
        game.standing := outcome.crashed
    when:
        stepped = catch_up(mutable game, start, start + 300ms)
    then:
        last_step = (stepped == start)
        head = (game.head == cell(column: 10, row: 7))


-- Less than a step time after its last step, the snake does not step.
test the_snake_waits_for_a_whole_step_time
        covers catch_up > not_due'2:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
    when:
        stepped = catch_up(mutable game, start, start + 100ms)
    then:
        last_step = (stepped == start)
        head = (game.head == cell(column: 10, row: 7))


-- Exactly a step time after its last step, the snake steps once.
test the_snake_steps_once_a_step_time_exactly_after
        covers catch_up > due'2eq:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
    when:
        stepped = catch_up(mutable game, start, start + 150ms)
    then:
        last_step = (stepped == start + 150ms)
        head = (game.head == cell(column: 11, row: 7))


-- Between one and two step times after its last step, the snake steps once.
test the_snake_steps_once_between_one_and_two_step_times
        covers catch_up > due'2gt:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
    when:
        stepped = catch_up(mutable game, start, start + 200ms)
    then:
        last_step = (stepped == start + 150ms)
        head = (game.head == cell(column: 11, row: 7))


-- Exactly two step times after its last step, the snake steps twice.
test the_snake_steps_twice_two_step_times_exactly_after
        covers catch_up > due'many'2eq:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
    when:
        stepped = catch_up(mutable game, start, start + 300ms)
    then:
        last_step = (stepped == start + 300ms)
        head = (game.head == cell(column: 12, row: 7))


-- Between two and three step times after its last step, the snake steps twice.
test the_snake_steps_twice_between_two_and_three_step_times
        covers catch_up > due'many'2gt:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable game = moving(direction.right)
    when:
        stepped = catch_up(mutable game, start, start + 350ms)
    then:
        last_step = (stepped == start + 300ms)
        head = (game.head == cell(column: 12, row: 7))
