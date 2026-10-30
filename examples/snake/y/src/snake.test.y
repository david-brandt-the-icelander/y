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
        covers new_game, free_cells > row'many > column'many > vacant, free_cells > row'many > column'many > taken'1,
               free_cells > row'many > column'many > taken'2, place_food > room:
    inject:
            random = fake random(seed: 7)
    when:
        started = new_game()
    then:
        three_long = (started.tail.count() == 2)
        moving_right = (started.heading == direction.right)
        food_off_the_snake = ((started.food != started.head) and (not started.tail.contains(started.food)))


-- The snake moves one cell at a time, each of the four ways.
test the_snake_moves_one_cell_the_way_it_heads
        covers ahead_of > going.up, ahead_of > going.left:
    given:
        mutable up = moving(direction.up)
        mutable down = moving(direction.down)
        mutable left = moving(direction.left)
        mutable right = moving(direction.right)
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable up)
        advance(mutable down)
        advance(mutable left)
        advance(mutable right)
    then:
        moved_up = (up.head == cell(column: 10, row: 6))
        moved_down = (down.head == cell(column: 10, row: 8))
        moved_left = (left.head == cell(column: 9, row: 7))
        moved_right = (right.head == cell(column: 11, row: 7))
        still_playing = (right.standing == outcome.playing)


-- The tail follows the head, and the snake keeps its length.
test the_tail_follows_the_head
        covers advance > moved.on, ahead_of > going.right, on_board:
    given:
        mutable game = moving(direction.right)
        game.tail := [cell(column: 9, row: 7), cell(column: 8, row: 7)]
    inject:
            random = fake random(seed: 7)
    when:
        advance(mutable game)
    then:
        followed = (game.tail == [cell(column: 10, row: 7), cell(column: 9, row: 7)])


-- Each arrow key, and each of W, A, S and D, turns the snake a way it is not heading back along.
test keys_turn_the_snake
        covers turned > arrow.up'1, turned > arrow.down'2, turned > arrow.right'2, opposite > going.up,
               opposite > going.down, opposite > going.left:
    given:
        heading_right = moving(direction.right)
        heading_left = moving(direction.left)
        heading_down = moving(direction.down)
        heading_up = moving(direction.up)
    when:
        up = turned(heading_right, key.up)
        down = turned(heading_left, key.s)
        left = turned(heading_down, key.left)
        right = turned(heading_up, key.d)
    then:
        turned_up = (up == direction.up)
        turned_down = (down == direction.down)
        turned_left = (left == direction.left)
        turned_right = (right == direction.right)


-- A key pointing back the way the snake moved last does not turn it, and nor does a key that is not an arrow.
test the_snake_does_not_turn_back_on_itself
        covers turned > arrow.left'1, turned > arrow.other, unless_reversing > reversing, opposite > going.right:
    given:
        mutable game = moving(direction.right)
        game.wanted := direction.up
    when:
        back = turned(game, key.left)
        other = turned(game, key.space)
    then:
        -- Expected the turn the player made before to stand.
        still_up = (back == direction.up)
        unchanged = (other == direction.up)


-- Moving into the wall ends the game, and leaves the snake where it was.
test the_wall_ends_the_game
        covers advance > moved.into_wall, banner > shown.crashed:
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
        told = (banner(game, 0) == "Crashed into the wall with 0. Space plays again.")


-- Moving into the snake's own body ends the game.
test the_snake_cannot_bite_itself
        covers advance > moved.into_itself, banner > shown.bitten:
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
        told = (banner(game, 0) == "Bit itself with 0. Space plays again.")


-- The head may move into the cell the end of the tail is leaving.
test the_snake_may_follow_its_own_tail
        covers ahead_of > going.down:
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
        told = (banner(game, 1) == "Score 1   Best 1   Arrows steer, Escape quits")


-- When the snake eats the last food there is room for, it fills the board, and the game ends.
test filling_the_board_ends_the_game
        covers place_food > full, banner > shown.filled:
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
        filled = (game.standing == outcome.filled)
        told = (banner(game, 298) == "Filled the board with 1! Space plays again.")


-- The game is drawn in each frame, and the snake steps once for each step time that passes, until the window closes.
test the_game_plays_until_the_window_closes
        covers draw > part'many, play > open'many > going_on'1, steer > no_keys, catch_up > due'2eq:
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
        covers play > open'many > again, play > open'many > going_on'2:
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


-- Escape ends play at the end of the frame it is pressed in.
test escape_quits
        covers play > open > going_on'1, steer > pressed:
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


-- W, A and S, and the arrows the other test does not press, turn the snake too.
test the_other_keys_turn_the_snake
        covers turned > arrow.up'2, turned > arrow.down'1, turned > arrow.left'2, turned > arrow.right'1:
    given:
        heading_right = moving(direction.right)
        heading_left = moving(direction.left)
        heading_down = moving(direction.down)
        heading_up = moving(direction.up)
    when:
        up = turned(heading_right, key.w)
        down = turned(heading_left, key.down)
        left = turned(heading_down, key.a)
        right = turned(heading_up, key.right)
    then:
        turned_up = (up == direction.up)
        turned_down = (down == direction.down)
        turned_left = (left == direction.left)
        turned_right = (right == direction.right)


-- Two keys pressed in one frame steer in the order they were pressed.
test two_keys_in_a_frame_steer_in_turn
        covers unless_reversing > turning, steer > pressed'many:
    given:
        mutable game = moving(direction.right)
    when:
        steer(mutable game, [key.up, key.left])
    then:
        -- Expected up to stand: left, after up, is back the way the snake moved last.
        up = (game.wanted == direction.up)


-- The snake steps once for each step time passed since its last step, and not at all when the game
-- is over.
test the_snake_catches_up_with_the_time
        covers catch_up > not_due'1, catch_up > not_due'2, catch_up > due'2gt, catch_up > due'many'2eq,
               catch_up > due'many'2gt:
    inject:
            clock = fake clock(tick: 1s)
            random = fake random(seed: 7)
    given:
        start = clock.now()
        mutable early = moving(direction.right)
        mutable once = moving(direction.right)
        mutable twice = moving(direction.right)
        mutable over = moving(direction.right)
        over.standing := outcome.crashed
    when:
        not_yet = catch_up(mutable early, start, start + 100ms)
        one_step = catch_up(mutable once, start, start + 200ms)
        two_steps = catch_up(mutable twice, start, start + 300ms)
        none = catch_up(mutable over, start, start + 300ms)
    then:
        early_stays = ((not_yet == start) and (early.head == cell(column: 10, row: 7)))
        once_moves = ((one_step == start + 150ms) and (once.head == cell(column: 11, row: 7)))
        twice_moves = ((two_steps == start + 300ms) and (twice.head == cell(column: 12, row: 7)))
        over_stays = ((none == start) and (over.head == cell(column: 10, row: 7)))


-- A snake with no tail, and one with a tail of one cell, are drawn.
test a_snake_of_one_or_two_cells_is_drawn
        covers banner > shown.playing, draw > no_tail, draw > part, fill_cell:
    inject:
            window = fake window(frames: [])
    given:
        alone = moving(direction.right)
        mutable short = moving(direction.right)
        short.tail := [cell(column: 9, row: 7)]
    when:
        draw(alone, 0)
        draw(short, 0)
    then:
        both_written = (window.written() == ["Score 0   Best 0   Arrows steer, Escape quits", "Score 0   Best 0   Arrows steer, Escape quits"])


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
        no_score = (best == 0)
