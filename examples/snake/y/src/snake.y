module snake

from standard.random import random
from standard.time import clock, duration
from standard.wait import forever
from standard.window import colour, key, window


-- The board, the rules, and how a game starts.
public facet setup:
    -- The board the snake moves on, in cells.
    constants board:
        columns is int32 = 20
        rows is int32 = 15


    -- The rules every game is played by.
    constants game_rules:
        -- How long the snake takes to move one cell.
        step_time is duration = 150ms


    -- One cell of the board, counted from 0 at the top left.
    structure cell:
        column is int32
        row is int32


    -- Which way the snake moves.
    constants direction:
        up = "up"
        down = "down"
        left = "left"
        right = "right"


    -- Where a game stands.
    constants outcome:
        playing = "playing"
        crashed = "crashed into the wall"
        bitten = "bit itself"
        filled = "filled the board"


    -- One game.
    structure game:
        -- The cell the snake's head is in.
        head is cell

        -- The cells of the rest of the snake, the one nearest the head first.
        tail is list<cell>

        -- The way the snake moved last.
        heading is direction

        -- The way it will move next, as the player last steered it.
        wanted is direction

        -- The cell the food is in.
        food is cell

        -- How much food the snake has eaten: the score.
        eaten is uint32
        standing is outcome


    ----------------------------------------------------------------------------------------------------------------------------
    Starts a game: a snake three cells long in the left half of the board, moving right, and food in a free cell
    chosen at random.

    started: The game, as game.

    random: What chooses the food's cell, as random.
    ----------------------------------------------------------------------------------------------------------------------------
    function new_game(injected random) returns started:
        mutable fresh = game(
            head: cell(column: 6, row: 7),
            tail: [cell(column: 5, row: 7), cell(column: 4, row: 7)],
            heading: direction.right,
            wanted: direction.right,

            -- Until `place_food` chooses a free cell.
            food: cell(column: 6, row: 7),
            eaten: 0,
            standing: outcome.playing,
        )
        place_food(mutable fresh)
        return started = fresh


-- How the player steers the snake, and how it moves.
public facet rules:
    ----------------------------------------------------------------------------------------------------------------------------
    Turns the snake the way an arrow key, or one of W, A, S and D, points. The snake cannot turn back on itself, so a
    key pointing back the way it moved last leaves it going as the player last steered it.

    game: The game, as game.
    pressed: The key the player pressed, as key.

    wanted: The way the snake will move next, as direction.
    ----------------------------------------------------------------------------------------------------------------------------
    function turned(game, pressed) returns wanted:
        conditional arrow = pressed:
            condition arrow.up = ((arrow == key.up) or (arrow == key.w)):
                return wanted = unless_reversing(game, direction.up)
            condition arrow.down = ((arrow == key.down) or (arrow == key.s)):
                return wanted = unless_reversing(game, direction.down)
            condition arrow.left = ((arrow == key.left) or (arrow == key.a)):
                return wanted = unless_reversing(game, direction.left)
            condition arrow.right = ((arrow == key.right) or (arrow == key.d)):
                return wanted = unless_reversing(game, direction.right)
            else arrow.other:
                return wanted = game.wanted


    ----------------------------------------------------------------------------------------------------------------------------
    Moves the snake one cell the way the player last steered it. Moving into the wall or into the snake's own body ends
    the game. Moving onto the food makes the snake one cell longer, and puts new food in a free cell.

    game: The game, which changes, as game.

    random: What chooses the new food's cell, as random.
    ----------------------------------------------------------------------------------------------------------------------------
    function advance(mutable game, injected random)
            where still_playing = (game.standing == outcome.playing):
        game.heading := game.wanted
        ahead = ahead_of(game.head, game.heading)

        conditional moved:
            condition moved.into_wall = (not on_board(ahead)):
                game.standing := outcome.crashed

            -- The last cell of the tail moves on as the head moves in, so only the cells before it are in the way.
            condition moved.into_itself = game.tail.without_last().contains(ahead):
                game.standing := outcome.bitten
            condition moved.onto_food = (ahead == game.food):
                game.tail.insert_first(game.head)
                game.head := ahead
                game.eaten := game.eaten + 1
                place_food(mutable game)
            else moved.on:
                game.tail.insert_first(game.head)
                game.head := ahead
                game.tail.remove_last()


--------------------------------------------------------------------------------------------------------------------------------
The way the snake will move next, unless the player is steering it back the way it moved last.

game: The game, as game.
toward: The way the player steered, as direction.

wanted: `toward`, or the way the player steered before when `toward` turns the snake back on itself, as direction.
--------------------------------------------------------------------------------------------------------------------------------
function unless_reversing(game, toward) returns wanted:
    if reversing = (toward == opposite(game.heading)):
        return wanted = game.wanted
    else turning:
        return wanted = toward


--------------------------------------------------------------------------------------------------------------------------------
The way back.

heading: A way to move, as direction.

back: The opposite way, as direction.
--------------------------------------------------------------------------------------------------------------------------------
function opposite(heading) returns back:
    conditional going = heading:
        condition going.up = (going == direction.up):
            return back = direction.down
        condition going.down = (going == direction.down):
            return back = direction.up
        condition going.left = (going == direction.left):
            return back = direction.right
        condition going.right = (going == direction.right):
            return back = direction.left


--------------------------------------------------------------------------------------------------------------------------------
The next cell one way. It may be off the board.

here: The cell to move from, as cell.
heading: The way to move, as direction.

ahead: The cell next to `here` that way, as cell.
--------------------------------------------------------------------------------------------------------------------------------
function ahead_of(here, heading) returns ahead:
    conditional going = heading:
        condition going.up = (going == direction.up):
            return ahead = cell(column: here.column, row: here.row - 1)
        condition going.down = (going == direction.down):
            return ahead = cell(column: here.column, row: here.row + 1)
        condition going.left = (going == direction.left):
            return ahead = cell(column: here.column - 1, row: here.row)
        condition going.right = (going == direction.right):
            return ahead = cell(column: here.column + 1, row: here.row)


--------------------------------------------------------------------------------------------------------------------------------
Whether a cell is on the board.

place: The cell, as cell.

inside: Whether it is, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function on_board(place) returns inside:
    return inside = ((place.column >= 0) and (place.column < board.columns) and (place.row >= 0) and (place.row < board.rows))


--------------------------------------------------------------------------------------------------------------------------------
The cells the snake is not in, row by row from the top left.

game: The game, as game.

free: The cells, as list<cell>.
--------------------------------------------------------------------------------------------------------------------------------
function free_cells(game) returns free:
    mutable found = list.new()

    for row in range(from: 0, below: board.rows):
        for column in range(from: 0, below: board.columns):
            place = cell(column: column, row: row)

            if taken = ((place == game.head) or game.tail.contains(place)):
                continue
            else vacant:
                found.push(place)
    return free = found


--------------------------------------------------------------------------------------------------------------------------------
Puts the food in a free cell chosen at random. When there is none, the snake fills the board, and the game ends.

game: The game, which changes, as game.

random: What chooses the cell, as random.
--------------------------------------------------------------------------------------------------------------------------------
function place_food(mutable game, injected random):
    free = free_cells(game)

    if full = free.is_empty():
        game.standing := outcome.filled
    else room:
        game.food := free[random.index_below(free.count())]


-- What the player sees of a game.
public facet drawing:
    -- The sizes of what is drawn, in pixels.
    constants layout:
        -- The side of one cell.
        cell_size is int32 = 32

        -- The height of the bar beneath the board, where the score is written.
        bar_height is int32 = 40
        width is int32 = board.columns * layout.cell_size
        height is int32 = board.rows * layout.cell_size + layout.bar_height


    -- The colours of what is drawn.
    constants palette:
        background is colour = colour(red: 24, green: 26, blue: 33)
        board is colour = colour(red: 36, green: 42, blue: 52)
        head is colour = colour(red: 140, green: 230, blue: 110)
        body is colour = colour(red: 70, green: 170, blue: 80)
        food is colour = colour(red: 235, green: 85, blue: 70)
        text is colour = colour(red: 230, green: 230, blue: 230)


    ----------------------------------------------------------------------------------------------------------------------------
    The line written beneath the board: the score, and what the player can do.

    game: The game, as game.
    highest: The best score so far, as uint32.

    line: The text, as string.
    ----------------------------------------------------------------------------------------------------------------------------
    function banner(game, highest) returns line:
        conditional shown = game.standing:
            condition shown.playing = (shown == outcome.playing):
                return line = "Score {game.eaten}   Best {highest}   Arrows steer, Escape quits"
            condition shown.crashed = (shown == outcome.crashed):
                return line = "Crashed into the wall with {game.eaten}. Space plays again."
            condition shown.bitten = (shown == outcome.bitten):
                return line = "Bit itself with {game.eaten}. Space plays again."
            condition shown.filled = (shown == outcome.filled):
                return line = "Filled the board with {game.eaten}! Space plays again."


    ----------------------------------------------------------------------------------------------------------------------------
    Draws a game: the board, the food, the snake, and the banner beneath.

    game: The game, as game.
    highest: The best score so far, as uint32.

    window: Where the game is drawn, as window.
    ----------------------------------------------------------------------------------------------------------------------------
    function draw(game, highest, injected window):
        board_height = board.rows * layout.cell_size
        window.clear(palette.background)
        window.fill(x: 0, y: 0, width: layout.width, height: board_height, colour: palette.board)
        fill_cell(game.food, palette.food)

        for part in game.tail:
            fill_cell(part, palette.body)
        fill_cell(game.head, palette.head)
        line = banner(game, highest)
        window.write(text: line, x: 12, y: layout.height - 14, size: 24, colour: palette.text)


--------------------------------------------------------------------------------------------------------------------------------
Fills one cell, leaving a pixel's gap on each side so that the cells stand apart.

place: The cell, as cell.
shade: Its colour, as colour.

window: Where it is drawn, as window.
--------------------------------------------------------------------------------------------------------------------------------
function fill_cell(place, shade, injected window):
    x = place.column * layout.cell_size + 1
    y = place.row * layout.cell_size + 1
    window.fill(x: x, y: y, width: layout.cell_size - 2, height: layout.cell_size - 2, colour: shade)


-- Playing in a window, one game after another, until the window is closed.
public facet window_play:
    ----------------------------------------------------------------------------------------------------------------------------
    Plays Snake in a window: one game after another, until the player closes the window or presses Escape. When a
    game ends, Space starts the next.

    best: The best score of the games played, as uint32.

    window: Where the games are drawn and the keys are read, as window.
    clock: What times the snake's steps, as clock.
    random: What places the food, as random.
    forever: The right to wait for the next frame as long as it takes, as forever.
    ----------------------------------------------------------------------------------------------------------------------------
    function play(injected window, injected clock, injected random, injected forever) returns best:
        mutable game = new_game()
        mutable highest = 0
        mutable last_step = clock.now()
        mutable quitting = false

        while open = ((not quitting) and (not window.close_requested())):
            keys = window.keys_pressed()
            now = clock.now()
            quitting := keys.contains(key.escape)

            for pressed in keys:
                game.wanted := turned(game, pressed)

            if again = ((game.standing != outcome.playing) and keys.contains(key.space)):
                game := new_game()
                last_step := now
            else going_on:
                -- One step for each step time that has passed since the last, so that a slow frame does not slow the snake.
                while due = ((game.standing == outcome.playing) and (now - last_step >= game_rules.step_time)):
                    advance(mutable game)
                    last_step := last_step + game_rules.step_time
                else caught_up:
                    highest := highest.max(game.eaten)
            draw(game, highest)
            window.next_frame() forever because "a hidden window shows no frames, and the game waits until it is shown"
        else closed:
            return best = highest
