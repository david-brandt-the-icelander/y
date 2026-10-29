----------------------------------------------------------------------------------------------------------------------------
A game with a snake one cell long in the middle of the board, moving one way, and the food in the top left corner.

heading: The way the snake moves, as direction.

started: The game, as game.
----------------------------------------------------------------------------------------------------------------------------
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
        covers place_food#room, free_cells#taken, free_cells#vacant:
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
        covers advance#moved.on, ahead_of#going.up, ahead_of#going.down, ahead_of#going.left, ahead_of#going.right:
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
        covers advance#moved.on:
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
        covers turned#arrow.up, turned#arrow.down, turned#arrow.left, turned#arrow.right, unless_reversing#turning,
        opposite#going.up, opposite#going.down, opposite#going.left, opposite#going.right:
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
        covers unless_reversing#reversing, turned#arrow.other:
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
        covers advance#moved.into_wall, banner#shown.crashed:
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
        covers advance#moved.into_itself, banner#shown.bitten:
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
        covers advance#moved.on:
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
        covers advance#moved.onto_food, banner#shown.playing:
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
        covers place_food#full, banner#shown.filled:
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
        covers play#open, play#going_on, play#due, play#caught_up, play#closed:
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
        covers play#again:
    inject:
            -- Fourteen steps right take the snake from column 6 into the wall; Space then starts again.
            window = fake window(frames: [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [key.space], []])
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
        covers play#open, play#closed:
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
