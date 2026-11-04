module checkers

from standard.random import random
from standard.wait import forever
from standard.window import colour, key, point, window


-- The board, the pieces, the levels of play, and how a game starts.
public facet setup:
    -- The board.
    constants board:
        -- How many squares there are along each side of the board.
        size is int32 = 8

        -- How many rows of men each player starts with.
        men_rows is int32 = 3


    -- The rules every game is played by.
    constants game_rules:
        -- How many turns in a row may pass without a capture before the game is drawn.
        quiet_limit is uint32 = 80


    -- How a game won is scored.
    constants scoring:
        -- What a win scores before the player's moves are taken off, and before the level multiplies it.
        win_points is uint32 = 1000

        -- What each of the player's moves takes off.
        points_per_move is uint32 = 10

        -- The least a win scores, however many moves it took, before the level multiplies it.
        least_win is uint32 = 100


    -- The two players. The person plays red, at the bottom of the board, and moves first; the computer plays white.
    constants side:
        red = "red"
        white = "white"


    -- One piece on the board.
    structure piece:
        -- The player whose piece it is.
        owner is side

        -- Whether it has been crowned: a king moves backwards as well as forwards.
        king is boolean


    -- A square of the board, its row and its column each counted from 0 at the top left.
    structure square:
        row is int32
        column is int32


    -- A piece and the square it stands on.
    structure placed:
        square is square
        piece is piece


    -- A diagonal way across the board: how many rows and how many columns one step moves.
    structure direction:
        rows is int32
        columns is int32


    -- The ways a piece may move.
    constants directions:
        -- Towards the top of the board, the way red's men move.
        up is list<element=direction> = [direction(rows: -1, columns: -1), direction(rows: -1, columns: 1)]

        -- Towards the bottom of the board, the way white's men move.
        down is list<element=direction> = [direction(rows: 1, columns: -1), direction(rows: 1, columns: 1)]

        -- Every way, as kings move.
        all is list<element=direction> = [
            direction(rows: -1, columns: -1),
            direction(rows: -1, columns: 1),
            direction(rows: 1, columns: -1),
            direction(rows: 1, columns: 1),
        ]


    -- One move: a step to a square next to the piece, or a chain of one or more jumps, each over a piece of the other
    -- player's, which is captured.
    structure move:
        -- The piece that moves.
        piece is piece

        -- The square it moves from.
        from is square

        -- The squares it lands on, in order; the last is where it ends.
        path is list<element=square>

        -- The squares of the pieces it jumps over, in order: none for a step.
        captures is list<element=square>


    -- How hard the computer plays.
    structure level:
        -- What the player sees it called.
        name is string

        -- How many positions the computer may look at to choose each of its moves.
        checks is uint32

        -- How many times over a win at this level scores.
        weight is uint32


    -- The levels the player may choose from.
    constants levels:
        -- From the easiest to the hardest. Each looks at ten times as many positions as the one before.
        all is list<element=level> = [
            level(name: "Beginner", checks: 25, weight: 1),
            level(name: "Casual", checks: 250, weight: 2),
            level(name: "Club", checks: 2500, weight: 3),
            level(name: "Expert", checks: 25000, weight: 4),
            level(name: "Master", checks: 250000, weight: 5),
        ]


    -- Where a game stands, for the person playing.
    constants outcome:
        playing = "playing"
        won = "won"
        lost = "lost"
        drawn = "drawn"


    -- One game.
    structure game:
        -- Every square, row by row from the top left: the piece on it, or none.
        squares is list<element=optional<value=piece>>

        -- Whose turn it is.
        to_move is side

        -- How hard the computer plays.
        level is level

        -- How many moves have been made, by both players.
        turns is uint32

        -- How many turns in a row have passed without a capture.
        quiet is uint32

        -- The move made last, if one has been.
        last is optional<value=move>


    ----------------------------------------------------------------------------------------------------------------------------
    Starts a game: twelve men for each player on the dark squares of the three rows nearest them, and red to move.

    level: How hard the computer plays, as level.

    started: The game, as game.
    ----------------------------------------------------------------------------------------------------------------------------
    function new_game(level) returns started:
        mutable squares = list.new()

        for row in range(from: 0, below: board.size):
            for column in range(from: 0, below: board.size):
                squares.push(starting_piece(square(row: row, column: column)))
        return started = game(squares: squares, to_move: side.red, level: level, turns: 0, quiet: 0, last: optional.none)


--------------------------------------------------------------------------------------------------------------------------------
The piece a square holds when a game starts.

square: The square, as square.

held: Its piece, or none, as optional<value=piece>.
--------------------------------------------------------------------------------------------------------------------------------
function starting_piece(square) returns held:
    conditional placed:
        condition placed.light = ((square.row + square.column) % 2 == 0):
            return held = optional.none
        condition placed.white_men = (square.row < board.men_rows):
            return held = optional.some(piece(owner: side.white, king: false))
        condition placed.red_men = (square.row >= board.size - board.men_rows):
            return held = optional.some(piece(owner: side.red, king: false))
        else placed.between:
            return held = optional.none


-- The moves a player may make, making them, and how a game ends and is scored.
public facet rules:
    ----------------------------------------------------------------------------------------------------------------------------
    The moves the player to move may make. A player who can capture must, and so may make only jumps.

    game: The game, as game.

    moves: The moves, as list<element=move>.
    ----------------------------------------------------------------------------------------------------------------------------
    function legal_moves(game) returns moves:
        jumps = all_jumps(game)

        if must_jump = (not jumps.is_empty()):
            return moves = jumps
        else no_jump:
            return moves = all_steps(game)


    ----------------------------------------------------------------------------------------------------------------------------
    Makes a move: the piece leaves its square, the pieces it jumps over are taken off the board, and a man that ends
    on the far row is crowned. Then it is the other player's turn.

    game: The game, which changes, as game.
    chosen: The move, as move.
    ----------------------------------------------------------------------------------------------------------------------------
    function apply(mutable game, chosen)
            where own_piece = (chosen.piece.owner == game.to_move):
        to = chosen.path.last()
        crowned = piece(owner: chosen.piece.owner, king: chosen.piece.king or (to.row == far_row(chosen.piece.owner)))
        game.squares.set_number(number_of(chosen.from), optional.none)

        for captured in chosen.captures:
            game.squares.set_number(number_of(captured), optional.none)
            game.quiet := 0
        else quiet:
            game.quiet := game.quiet + 1
        game.squares.set_number(number_of(to), optional.some(crowned))
        game.turns := game.turns + 1
        game.to_move := other(game.to_move)
        game.last := optional.some(chosen)


    ----------------------------------------------------------------------------------------------------------------------------
    Where a game stands, for the person playing red. A player whose turn it is and who cannot move has lost, and a
    game in which too many turns pass without a capture is drawn.

    game: The game, as game.

    reached: Where it stands, as outcome.
    ----------------------------------------------------------------------------------------------------------------------------
    function standing(game) returns reached:
        stuck = legal_moves(game).is_empty()

        conditional judged:
            condition judged.won = (stuck and (game.to_move == side.white)):
                return reached = outcome.won
            condition judged.lost = stuck:
                return reached = outcome.lost
            condition judged.drawn = (game.quiet >= game_rules.quiet_limit):
                return reached = outcome.drawn
            else judged.playing:
                return reached = outcome.playing


    ----------------------------------------------------------------------------------------------------------------------------
    The score of a game: nothing unless the person won it. A win scores the win points, less some for each of the
    person's moves, but never less than the least win, and all of it times the level's weight.

    game: The game, as game.

    points: The score, as uint32.
    ----------------------------------------------------------------------------------------------------------------------------
    function score(game) returns points:
        if won = (standing(game) == outcome.won):
            taken = (player_moves(game) * scoring.points_per_move).min(scoring.win_points - scoring.least_win)
            return points = (scoring.win_points - taken) * game.level.weight
        else unfinished:
            return points = 0


    ----------------------------------------------------------------------------------------------------------------------------
    How many moves the person has made. Red moves first, so red has made every odd turn.

    game: The game, as game.

    made: The count, as uint32.
    ----------------------------------------------------------------------------------------------------------------------------
    function player_moves(game) returns made:
        return made = game.turns.divided_rounding_up(by: 2)


    ----------------------------------------------------------------------------------------------------------------------------
    The player who moves after `player`.

    player: A side, as side.

    next: The other side, as side.
    ----------------------------------------------------------------------------------------------------------------------------
    function other(player) returns next:
        conditional current = player:
            condition current.red = (current == side.red):
                return next = side.white
            condition current.white = (current == side.white):
                return next = side.red


--------------------------------------------------------------------------------------------------------------------------------
The steps the player to move may make: to an empty square next to a piece, the ways it may move.

game: The game, as game.

found: The steps, as list<element=move>.
--------------------------------------------------------------------------------------------------------------------------------
function all_steps(game) returns found:
    mutable steps = list.new()

    for placed in own_pieces(game):
        for direction in directions_of(placed.piece):
            to = square(row: placed.square.row + direction.rows, column: placed.square.column + direction.columns)

            if open = (on_board(to) and (piece_at(game, to) == optional.none)):
                steps.push(move(piece: placed.piece, from: placed.square, path: [to], captures: []))
            else blocked:
                continue
    else no_pieces
    return found = steps


--------------------------------------------------------------------------------------------------------------------------------
The jumps the player to move may make, each as long a chain as it can go.

game: The game, as game.

found: The jumps, as list<element=move>.
--------------------------------------------------------------------------------------------------------------------------------
function all_jumps(game) returns found:
    mutable jumps = list.new()

    for placed in own_pieces(game):
        jumps.extend(chains(game, placed.piece, placed.square, placed.square, [], []))
    else no_pieces
    return found = jumps


--------------------------------------------------------------------------------------------------------------------------------
The chains of jumps a piece can go on with from a square it has reached. A chain goes on for as long as the piece can
jump again, and ends when a man is crowned. A piece is not jumped twice, and the square the chain started from counts
as empty, as the piece has left it.

game: The game, as it was before the chain started, as game.
piece: The piece that jumps, as piece.
start: The square the chain started from, as square.
at: The square the piece has reached, as square.
path: The squares it has landed on so far, as list<element=square>.
captured: The squares of the pieces it has jumped over so far, as list<element=square>.

found: The chains, each a whole move, or none when it cannot jump from `at`, as list<element=move>.
--------------------------------------------------------------------------------------------------------------------------------
function chains(game, piece, start, at, path, captured) returns found:
    mutable moves = list.new()

    for direction in directions_of(piece):
        over = square(row: at.row + direction.rows, column: at.column + direction.columns)
        land = square(row: over.row + direction.rows, column: over.column + direction.columns)

        if can_jump = (jumpable(game, piece, start, over, land) and (not captured.contains(over))):
            mutable longer = path
            longer.push(land)
            mutable taken = captured
            taken.push(over)
            further = chains_after(game, piece, start, land, longer, taken)

            if ends_here = further.is_empty():
                moves.push(move(piece: piece, from: start, path: longer, captures: taken))
            else goes_on:
                moves.extend(further)
        else cannot_jump:
            continue
    return found = moves


--------------------------------------------------------------------------------------------------------------------------------
The chains of jumps that go on from a square a piece has just landed on: none when the landing crowns a man.

game: The game, as it was before the chain started, as game.
piece: The piece that jumps, as piece.
start: The square the chain started from, as square.
land: The square just landed on, as square.
path: The squares it has landed on so far, as list<element=square>.
captured: The squares of the pieces it has jumped over so far, as list<element=square>.

found: The chains, as list<element=move>.
--------------------------------------------------------------------------------------------------------------------------------
function chains_after(game, piece, start, land, path, captured) returns found:
    if crowned = ((not piece.king) and (land.row == far_row(piece.owner))):
        return found = []
    else uncrowned:
        return found = chains(game, piece, start, land, path, captured)


--------------------------------------------------------------------------------------------------------------------------------
Whether a piece may jump over one square onto the next: the square jumped over holds the other player's piece, and
the square landed on is on the board and empty.

game: The game, as it was before the chain started, as game.
piece: The piece that jumps, as piece.
start: The square the chain started from, which counts as empty, as square.
over: The square jumped over, as square.
land: The square landed on, as square.

can: Whether it may, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function jumpable(game, piece, start, over, land) returns can:
    if inside = on_board(land):
        free = ((piece_at(game, land) == optional.none) or (land == start))
        return can = (belongs_to(game, over, other(piece.owner)) and free)
    else outside:
        return can = false


--------------------------------------------------------------------------------------------------------------------------------
Whether a square holds a piece of a player's.

game: The game, as game.
square: The square, on the board, as square.
owner: The player, as side.

holds: Whether it does, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function belongs_to(game, square, owner) returns holds:
    conditional held = piece_at(game, square):
        condition held.empty = (held == optional.none):
            return holds = false
        else held.piece:
            return holds = (held.owner == owner)


--------------------------------------------------------------------------------------------------------------------------------
Every piece on the board, with its square, row by row from the top left.

game: The game, as game.

found: The pieces, as list<element=placed>.
--------------------------------------------------------------------------------------------------------------------------------
function placed_pieces(game) returns found:
    mutable pieces = list.new()

    for row in range(from: 0, below: board.size):
        for column in range(from: 0, below: board.size):
            here = square(row: row, column: column)

            conditional held = piece_at(game, here):
                condition held.empty = (held == optional.none):
                    continue
                else held.piece:
                    pieces.push(placed(square: here, piece: held))
    return found = pieces


--------------------------------------------------------------------------------------------------------------------------------
The pieces of the player to move, with their squares.

game: The game, as game.

found: The pieces, as list<element=placed>.
--------------------------------------------------------------------------------------------------------------------------------
function own_pieces(game) returns found:
    mutable pieces = list.new()

    for placed in placed_pieces(game):
        if mine = (placed.piece.owner == game.to_move):
            pieces.push(placed)
        else theirs:
            continue
    else no_pieces
    return found = pieces


--------------------------------------------------------------------------------------------------------------------------------
The ways a piece may move.

piece: The piece, as piece.

ways: The directions: two for a man, and four for a king, as list<element=direction>.
--------------------------------------------------------------------------------------------------------------------------------
function directions_of(piece) returns ways
        ensures several = (ways.count() >= 2):
    conditional moving:
        condition moving.king = piece.king:
            return ways = directions.all
        condition moving.red = (piece.owner == side.red):
            return ways = directions.up
        else moving.white:
            return ways = directions.down


--------------------------------------------------------------------------------------------------------------------------------
The row on which a player's men are crowned: the row farthest from where they start.

owner: The player, as side.

row: The row, as int32.
--------------------------------------------------------------------------------------------------------------------------------
function far_row(owner) returns row:
    if heading_up = (owner == side.red):
        return row = 0
    else heading_down:
        return row = board.size - 1


--------------------------------------------------------------------------------------------------------------------------------
The piece on a square, if there is one.

game: The game, as game.
square: The square, on the board, as square.

held: The piece, or none, as optional<value=piece>.
--------------------------------------------------------------------------------------------------------------------------------
function piece_at(game, square) returns held:
    return held = game.squares.at_number(number_of(square))


--------------------------------------------------------------------------------------------------------------------------------
The number of a square in a game's list of squares, counting from 1 row by row from the top left.

square: The square, on the board, as square.

number: The number, as uint32.
--------------------------------------------------------------------------------------------------------------------------------
function number_of(square) returns number:
    return number = (square.row * board.size + square.column + 1).to_uint32()


--------------------------------------------------------------------------------------------------------------------------------
Whether a square is on the board.

square: The square, as square.

inside: Whether it is, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function on_board(square) returns inside:
    rows = ((square.row >= 0) and (square.row < board.size))
    columns = ((square.column >= 0) and (square.column < board.size))
    return inside = (rows and columns)


-- How the computer chooses its moves.
public facet computer:
    -- What the computer's search counts.
    constants search:
        -- The most moves deep the minimax search goes.
        deepest is uint32 = 10

        -- The most moves a game played out at random runs for.
        playout_turns is uint32 = 12

        -- What a game won is worth, more than any count of pieces.
        win_value is int32 = 100

        -- What a man is worth.
        man_value is int32 = 3

        -- What a king is worth.
        king_value is int32 = 5


    -- A game played out at random from a position.
    structure played_out:
        -- What the position it ended in is worth.
        value is int32

        -- How many positions it looked at: one for each move made, and one for the position it ended in.
        spent is uint32


    ----------------------------------------------------------------------------------------------------------------------------
    Chooses the computer's move, looking at no more positions than its level allows: its checks.

    Half the checks go to a minimax search, which weighs each move by the pieces on the board when both players play
    their best for a number of moves ahead: one, then two, and so on, for as long as the checks last. The rest go to
    a Monte Carlo search among the moves the minimax search weighs best: each is worth the average of games played
    out at random from it.

    game: The game, as game.

    chosen: The move, as move.

    random: What orders the moves and plays the games out, as random.
    ----------------------------------------------------------------------------------------------------------------------------
    function computer_move(game, injected random) returns chosen
            where has_moves = (not legal_moves(game).is_empty()):
        options = random.shuffled(legal_moves(game))
        allowance = game.level.checks / 2
        mutable left = allowance
        mutable values = list.repeated(0, times: options.count())
        mutable depth = 1
        mutable exhausted = false

        while deepening = ((depth <= search.deepest) and (not exhausted)):
            mutable found = list.new()

            for candidate in options:
                mutable after = game
                apply(mutable after, candidate)
                found.push(searched(after, game.to_move, depth - 1, mutable left))

            if finished = (left > 0):
                values := found
                depth := depth + 1
            else ran_out:
                exhausted := true
        return chosen = best_sampled(game, options, values, game.level.checks - (allowance - left))


--------------------------------------------------------------------------------------------------------------------------------
What a position is worth to a player when both players play their best for a number of moves ahead: the minimax
value. Each position it reaches is a check; when the checks run out, it gives up, leaves none, and its value is not
to be used.

game: The position, as game.
owner: The player, as side.
depth: How many moves ahead to look, as uint32.
left: How many checks are left, which changes, as uint32.

value: The value, as int32.
--------------------------------------------------------------------------------------------------------------------------------
function searched(game, owner, depth, mutable left) returns value:
    moves = legal_moves(game)

    conditional judged:
        condition judged.leaf = ((depth == 0) or moves.is_empty() or (game.quiet >= game_rules.quiet_limit)):
            return value = value_for(game, owner)
        condition judged.spent = (moves.count() >= left):
            left := 0
            return value = 0
        else judged.inner:
            left := left - moves.count()
            mutable values = list.new()

            for candidate in moves:
                mutable after = game
                apply(mutable after, candidate)
                values.push(searched(after, owner, depth - 1, mutable left))

            if owners_turn = (game.to_move == owner):
                return value = values.largest()
            else others_turn:
                return value = values.smallest()


--------------------------------------------------------------------------------------------------------------------------------
The best of the moves the minimax search weighs best, by games played out at random from each within a share of the
checks. When one move is weighed best, it is chosen without them.

game: The game, as game.
options: The moves, as list<element=move>.
values: What the minimax search weighs each move at, in the same order, as list<element=int32>.
checks: How many positions the games may look at in all, as uint32.

chosen: The move, as move.

random: What plays the games out, as random.
--------------------------------------------------------------------------------------------------------------------------------
function best_sampled(game, options, values, checks, injected random) returns chosen
        where has_options = (not options.is_empty()):
    best = values.largest()
    mutable tied = list.new()

    for option in range(from: 1, through: options.count()):
        compare ranked = values.at_number(option) with best:
            condition ranked.top = equal:
                tied.push(option)
            condition ranked.lower = less:
                continue
            condition ranked.above = greater:
                fail not_reachable

    compare ties = tied.count() with 1:
        condition ties.settled = equal:
            return chosen = options.at_number(tied[0])
        condition ties.contested = greater:
            share = checks / tied.count()
            mutable averages = list.new()

            for option in tied:
                mutable after = game
                apply(mutable after, options.at_number(option))
                averages.push(sampled_value(after, game.to_move, share))
            return chosen = options.at_number(tied.at_number(averages.number_of(averages.largest())))
        condition ties.none = less:
            fail not_reachable


--------------------------------------------------------------------------------------------------------------------------------
What a position is worth to a player, from games played out from it at random within a share of the checks; or, when
there is no share, from the pieces on the board.

position: The position, as game.
owner: The player, as side.
share: How many positions the games may look at, as uint32.

value: The average of what the games' ends are worth, or the position's own worth, as int32.

random: What plays the games out, as random.
--------------------------------------------------------------------------------------------------------------------------------
function sampled_value(position, owner, share, injected random) returns value:
    if no_share = (share == 0):
        return value = value_for(position, owner)
    else some_share:
        mutable left = share
        mutable total = 0
        mutable played = 0

        while sampling = (left > 0):
            length = (left - 1).min(search.playout_turns)
            outcome = playout(position, owner, length)
            total := total + outcome.value
            played := played + 1
            left := left - outcome.spent
        return value = total / played


--------------------------------------------------------------------------------------------------------------------------------
Plays a game out from a position, each move chosen at random, until it ends or `length` moves are made.

start: The position, as game.
owner: The player whose worth the end is measured by, as side.
length: The most moves to make, as uint32.

played: What the end is worth, and how many positions were looked at, as played_out.

random: What chooses each move, as random.
--------------------------------------------------------------------------------------------------------------------------------
function playout(start, owner, length, injected random) returns played:
    mutable position = start
    mutable moves = legal_moves(position)
    mutable made = 0

    while going = ((made < length) and (not moves.is_empty()) and (position.quiet < game_rules.quiet_limit)):
        apply(mutable position, random.one_of(moves))
        moves := legal_moves(position)
        made := made + 1
    else ended
    return played = played_out(value: value_for(position, owner), spent: made + 1)


--------------------------------------------------------------------------------------------------------------------------------
What a position is worth to a player: the win value for a win, less it for a loss, nothing for a draw, and otherwise
what the player's pieces are worth less what the other player's are.

game: The position, as game.
owner: The player, as side.

worth: The worth, as int32.
--------------------------------------------------------------------------------------------------------------------------------
function value_for(game, owner) returns worth:
    stuck = legal_moves(game).is_empty()

    conditional judged:
        condition judged.losing = (stuck and (game.to_move == owner)):
            return worth = -search.win_value
        condition judged.winning = stuck:
            return worth = search.win_value
        condition judged.drawn = (game.quiet >= game_rules.quiet_limit):
            return worth = 0
        else judged.playing:
            return worth = material(game, owner) - material(game, other(owner))


--------------------------------------------------------------------------------------------------------------------------------
What a player's pieces on the board are worth.

game: The game, as game.
owner: The player, as side.

worth: The worth, as int32.
--------------------------------------------------------------------------------------------------------------------------------
function material(game, owner) returns worth:
    mutable total = 0

    for placed in placed_pieces(game):
        conditional counted = placed.piece:
            condition counted.other = (counted.owner != owner):
                continue
            condition counted.king = counted.king:
                total := total + search.king_value
            else counted.man:
                total := total + search.man_value
    else no_pieces
    return worth = total


-- What the player sees.
public facet drawing:
    -- The sizes of what is drawn, in pixels.
    constants layout:
        -- The side of a square.
        square_size is int32 = 72

        -- The space around the board.
        gap is int32 = 12

        -- The side of the board.
        board_size is int32 = board.size * layout.square_size

        -- The radius of a piece, of the ring inside it, and of the mark of a square a piece may move to.
        piece_radius is int32 = 28
        ring_radius is int32 = 22
        centre_radius is int32 = 12
        mark_radius is int32 = 10

        -- The height of the bar beneath the board, where the score is written.
        bar_height is int32 = 84

        -- The height of the text, and of a title.
        text_size is uint32 = 20
        title_size is uint32 = 40

        -- Where the first button of the levels is, how high each is, and how far it is from one to the next.
        button_top is int32 = 140
        button_height is int32 = 72
        button_step is int32 = 88
        width is int32 = layout.gap + layout.board_size + layout.gap
        height is int32 = layout.gap + layout.board_size + layout.bar_height


    -- The colours of what is drawn.
    constants palette:
        background is colour = colour(red: 30, green: 28, blue: 26)
        light is colour = colour(red: 232, green: 214, blue: 178)
        dark is colour = colour(red: 118, green: 82, blue: 56)
        selected is colour = colour(red: 86, green: 136, blue: 84)
        last is colour = colour(red: 150, green: 112, blue: 60)
        mark is colour = colour(red: 140, green: 210, blue: 130)
        red is colour = colour(red: 196, green: 42, blue: 38)
        red_ring is colour = colour(red: 150, green: 28, blue: 26)
        white is colour = colour(red: 240, green: 236, blue: 224)
        white_ring is colour = colour(red: 196, green: 190, blue: 176)
        crown is colour = colour(red: 232, green: 190, blue: 60)
        button is colour = colour(red: 70, green: 64, blue: 58)
        text is colour = colour(red: 232, green: 230, blue: 226)
        muted is colour = colour(red: 170, green: 166, blue: 160)


    ----------------------------------------------------------------------------------------------------------------------------
    The lines written beneath the board: how the game stands, and what the player can do.

    game: The game, as game.
    highest: The best score so far, as uint32.

    lines: The two lines, as list<element=string>.
    ----------------------------------------------------------------------------------------------------------------------------
    function banner(game, highest) returns lines:
        moves = player_moves(game)
        name = game.level.name

        conditional shown = standing(game):
            condition shown.your_move = ((shown == outcome.playing) and (game.to_move == side.red)):
                return lines = [
                    "Your move   {name}   Moves {moves}   Best {highest}",
                    "Click a red piece, then a marked square. Escape quits.",
                ]
            condition shown.thinking = (shown == outcome.playing):
                return lines = [
                    "The computer is thinking, at {game.level.checks} positions a move...",
                    "Escape quits.",
                ]
            condition shown.won = (shown == outcome.won):
                return lines = [
                    "You won in {moves} moves against {name}! Score {score(game)}   Best {highest}",
                    "Space chooses a new game. Escape quits.",
                ]
            condition shown.lost = (shown == outcome.lost):
                return lines = [
                    "The computer won. Score 0   Best {highest}",
                    "Space chooses a new game. Escape quits.",
                ]
            condition shown.drawn = (shown == outcome.drawn):
                return lines = [
                    "A draw: {game_rules.quiet_limit} turns without a capture. Score 0   Best {highest}",
                    "Space chooses a new game. Escape quits.",
                ]


    ----------------------------------------------------------------------------------------------------------------------------
    Draws a game: the board, the pieces, the squares the selected piece may move to, and the banner beneath.

    game: The game, as game.
    selected: The square of the piece the player has selected, if there is one, as optional<value=square>.
    highest: The best score so far, as uint32.

    window: Where the game is drawn, as window.
    ----------------------------------------------------------------------------------------------------------------------------
    function draw(game, selected, highest, injected window):
        window.clear(palette.background)
        draw_board(game, selected)
        draw_pieces(game)
        draw_marks(game, selected)
        lines = banner(game, highest)
        top = layout.height - layout.bar_height
        window.write(text: lines[0], x: layout.gap, y: top + 32, size: layout.text_size, colour: palette.text)
        window.write(text: lines[1], x: layout.gap, y: top + 60, size: layout.text_size, colour: palette.muted)


    ----------------------------------------------------------------------------------------------------------------------------
    Draws the levels to choose from, each a button, and the best score so far.

    highest: The best score so far, as uint32.

    window: Where the levels are drawn, as window.
    ----------------------------------------------------------------------------------------------------------------------------
    function draw_levels(highest, injected window):
        window.clear(palette.background)
        window.write(text: "Checkers", x: layout.gap, y: 60, size: layout.title_size, colour: palette.text)
        prompt = "Click how hard the computer plays. Escape quits."
        window.write(text: prompt, x: layout.gap, y: 100, size: layout.text_size, colour: palette.muted)
        mutable top = layout.button_top

        for level in levels.all:
            checks = "Looks at {level.checks} positions a move. Wins score x{level.weight}."
            window.fill(x: layout.gap, y: top, width: layout.board_size, height: layout.button_height, colour: palette.button)
            window.write(text: level.name, x: layout.gap + 16, y: top + 30, size: layout.text_size, colour: palette.text)
            window.write(text: checks, x: layout.gap + 16, y: top + 56, size: layout.text_size, colour: palette.muted)
            top := top + layout.button_step
        window.write(text: "Best {highest}", x: layout.gap, y: top + 24, size: layout.text_size, colour: palette.text)


    ----------------------------------------------------------------------------------------------------------------------------
    The square under a point, if there is one.

    place: The point, as point.

    found: The square, or none when the point is off the board, as optional<value=square>.
    ----------------------------------------------------------------------------------------------------------------------------
    function square_at(place) returns found:
        across = ((place.x >= layout.gap) and (place.x < layout.gap + layout.board_size))
        down = ((place.y >= layout.gap) and (place.y < layout.gap + layout.board_size))

        if inside = (across and down):
            row = (place.y - layout.gap) / layout.square_size
            column = (place.x - layout.gap) / layout.square_size
            return found = optional.some(square(row: row, column: column))
        else outside:
            return found = optional.none


    ----------------------------------------------------------------------------------------------------------------------------
    The level whose button is under a point, if there is one.

    place: The point, as point.

    found: The level, or none when the point is on no button, as optional<value=level>.
    ----------------------------------------------------------------------------------------------------------------------------
    function level_at(place) returns found:
        mutable top = layout.button_top

        for level in levels.all:
            across = ((place.x >= layout.gap) and (place.x < layout.gap + layout.board_size))
            down = ((place.y >= top) and (place.y < top + layout.button_height))

            if pressed = (across and down):
                return found = optional.some(level)
            else missed:
                top := top + layout.button_step
        return found = optional.none


--------------------------------------------------------------------------------------------------------------------------------
The top left corner of a square.

square: The square, as square.

corner: The corner, as point.
--------------------------------------------------------------------------------------------------------------------------------
function corner_of(square) returns corner:
    return corner = point(x: layout.gap + square.column * layout.square_size, y: layout.gap + square.row * layout.square_size)


--------------------------------------------------------------------------------------------------------------------------------
The centre of a square.

square: The square, as square.

centre: The centre, as point.
--------------------------------------------------------------------------------------------------------------------------------
function centre_of(square) returns centre:
    corner = corner_of(square)
    return centre = point(x: corner.x + layout.square_size / 2, y: corner.y + layout.square_size / 2)


--------------------------------------------------------------------------------------------------------------------------------
The colour of a square: light or dark, or marked as the selected piece's or as part of the last move.

game: The game, as game.
square: The square, as square.
selected: The square of the piece the player has selected, if there is one, as optional<value=square>.

shade: The colour, as colour.
--------------------------------------------------------------------------------------------------------------------------------
function shade_of(game, square, selected) returns shade:
    conditional looks:
        condition looks.light = ((square.row + square.column) % 2 == 0):
            return shade = palette.light
        condition looks.selected = (selected == optional.some(square)):
            return shade = palette.selected
        condition looks.last = in_last_move(game, square):
            return shade = palette.last
        else looks.dark:
            return shade = palette.dark


--------------------------------------------------------------------------------------------------------------------------------
Whether a square is one the last move started from or landed on.

game: The game, as game.
square: The square, as square.

part: Whether it is, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function in_last_move(game, square) returns part:
    conditional made = game.last:
        condition made.none = (made == optional.none):
            return part = false
        else made.some:
            return part = ((made.from == square) or made.path.contains(square))


--------------------------------------------------------------------------------------------------------------------------------
The squares the selected piece may move to.

game: The game, as game.
selected: The square of the piece the player has selected, if there is one, as optional<value=square>.

found: The squares: none when no piece is selected, as list<element=square>.
--------------------------------------------------------------------------------------------------------------------------------
function landings(game, selected) returns found:
    mutable squares = list.new()

    for candidate in legal_moves(game):
        if from_selected = (optional.some(candidate.from) == selected):
            squares.push(candidate.path.last())
        else from_elsewhere:
            continue
    else no_moves
    return found = squares


--------------------------------------------------------------------------------------------------------------------------------
Draws the squares of the board.

game: The game, as game.
selected: The square of the piece the player has selected, if there is one, as optional<value=square>.

window: Where the board is drawn, as window.
--------------------------------------------------------------------------------------------------------------------------------
function draw_board(game, selected, injected window):
    for row in range(from: 0, below: board.size):
        for column in range(from: 0, below: board.size):
            here = square(row: row, column: column)
            corner = corner_of(here)
            shade = shade_of(game, here, selected)
            window.fill(x: corner.x, y: corner.y, width: layout.square_size, height: layout.square_size, colour: shade)


--------------------------------------------------------------------------------------------------------------------------------
Draws every piece on the board.

game: The game, as game.

window: Where the pieces are drawn, as window.
--------------------------------------------------------------------------------------------------------------------------------
function draw_pieces(game, injected window):
    for placed in placed_pieces(game):
        draw_piece(placed)
    else no_pieces


--------------------------------------------------------------------------------------------------------------------------------
Marks the squares the selected piece may move to.

game: The game, as game.
selected: The square of the piece the player has selected, if there is one, as optional<value=square>.

window: Where the marks are drawn, as window.
--------------------------------------------------------------------------------------------------------------------------------
function draw_marks(game, selected, injected window):
    for landing in landings(game, selected):
        centre = centre_of(landing)
        window.circle(x: centre.x, y: centre.y, radius: layout.mark_radius, colour: palette.mark)
    else no_marks


--------------------------------------------------------------------------------------------------------------------------------
Draws one piece: a disc in its player's colours, with a ring inside, and a crown at its centre if it is a king.

placed: The piece and its square, as placed.

window: Where the piece is drawn, as window.
--------------------------------------------------------------------------------------------------------------------------------
function draw_piece(placed, injected window):
    centre = centre_of(placed.square)
    colours = colours_of(placed.piece)
    window.circle(x: centre.x, y: centre.y, radius: layout.piece_radius, colour: colours[0])
    window.circle(x: centre.x, y: centre.y, radius: layout.ring_radius, colour: colours[1])
    window.circle(x: centre.x, y: centre.y, radius: layout.centre_radius, colour: colours[2])


--------------------------------------------------------------------------------------------------------------------------------
The colours a piece is drawn in.

piece: The piece, as piece.

colours: The colours of its disc, its ring and its centre, as list<element=colour>.
--------------------------------------------------------------------------------------------------------------------------------
function colours_of(piece) returns colours:
    conditional painted:
        condition painted.red_king = ((piece.owner == side.red) and piece.king):
            return colours = [palette.red, palette.red_ring, palette.crown]
        condition painted.red_man = (piece.owner == side.red):
            return colours = [palette.red, palette.red_ring, palette.red]
        condition painted.white_king = piece.king:
            return colours = [palette.white, palette.white_ring, palette.crown]
        else painted.white_man:
            return colours = [palette.white, palette.white_ring, palette.white]


-- Playing in a window, one game after another, until the window is closed.
public facet window_play:
    -- How a game played in the window ended.
    structure ending:
        -- The game's score.
        points is uint32

        -- Whether the player asked to stop playing.
        quitting is boolean


    ----------------------------------------------------------------------------------------------------------------------------
    Plays Checkers in a window: the player chooses a level, then plays a game against the computer, and so on, until
    the player closes the window or presses Escape.

    best: The best score of the games played, as uint32.

    window: Where the games are drawn and the clicks and keys are read, as window.
    random: What the computer's search draws on, as random.
    forever: The right to wait for the next frame as long as it takes, as forever.
    ----------------------------------------------------------------------------------------------------------------------------
    function play(injected window, injected random, injected forever) returns best:
        mutable highest = 0
        mutable quitting = false

        while open = (not quitting):
            conditional picked = choose_level(highest):
                condition picked.none = (picked == optional.none):
                    quitting := true
                else picked.level:
                    ended = play_game(new_game(picked), highest)
                    highest := highest.max(ended.points)
                    quitting := ended.quitting
        return best = highest


    ----------------------------------------------------------------------------------------------------------------------------
    Shows the levels until the player clicks one, presses Escape or closes the window.

    highest: The best score so far, as uint32.

    found: The level clicked, or none when the player stopped, as optional<value=level>.

    window: Where the levels are drawn and the clicks and keys are read, as window.
    forever: The right to wait for the next frame as long as it takes, as forever.
    ----------------------------------------------------------------------------------------------------------------------------
    function choose_level(highest, injected window, injected forever) returns found:
        mutable chosen = optional.none
        mutable quitting = false

        while choosing = ((chosen == optional.none) and (not quitting) and (not window.close_requested())):
            quitting := window.keys_pressed().contains(key.escape)
            chosen := level_clicked(window.clicks())
            draw_levels(highest)
            window.next_frame() forever because "a hidden window shows no frames, and the game waits until it is shown"
        else closed_at_once
        return found = chosen


    ----------------------------------------------------------------------------------------------------------------------------
    Plays a game, until it ends and the player presses Space, or the player presses Escape or closes the window.

    started: The game as it starts, as game.
    highest: The best score so far, as uint32.

    ended: How the game ended, as ending.

    window: Where the game is drawn and the clicks and keys are read, as window.
    random: What the computer's search draws on, as random.
    forever: The right to wait for the next frame as long as it takes, as forever.
    ----------------------------------------------------------------------------------------------------------------------------
    function play_game(started, highest, injected window, injected random, injected forever) returns ended:
        mutable game = started
        mutable selected = optional.none
        mutable quitting = false
        mutable again = false

        while playing = ((not quitting) and (not again) and (not window.close_requested())):
            keys = window.keys_pressed()
            quitting := keys.contains(key.escape)

            conditional moment:
                condition moment.over = (standing(game) != outcome.playing):
                    again := keys.contains(key.space)
                condition moment.computers_turn = (game.to_move == side.white):
                    chosen = computer_move(game)
                    apply(mutable game, chosen)
                else moment.players_turn:
                    act_on_clicks(mutable game, mutable selected, window.clicks())
            draw(game, selected, highest)
            window.next_frame() forever because "a hidden window shows no frames, and the game waits until it is shown"
        else closed_at_once
        return ended = ending(points: score(game), quitting: quitting)


    ----------------------------------------------------------------------------------------------------------------------------
    Acts on a click of the player's: a click on a square the selected piece may move to makes that move, a click on a
    piece of the player's that may move selects it, and any other click selects nothing.

    game: The game, which changes, as game.
    selected: The square of the piece the player has selected, if there is one, which changes, as optional<value=square>.
    place: The point clicked, as point.
    ----------------------------------------------------------------------------------------------------------------------------
    function clicked(mutable game, mutable selected, place):
        target = square_at(place)
        found = move_to(game, selected, target)

        conditional judged:
            condition judged.moving = (found != optional.none):
                apply(mutable game, found)
                selected := optional.none
            condition judged.choosing = may_move_from(game, target):
                selected := target
            else judged.elsewhere:
                selected := optional.none


--------------------------------------------------------------------------------------------------------------------------------
The level whose button the player clicked first, if any click was on one.

clicks: The points clicked, in order, as list<element=point>.

found: The level, or none, as optional<value=level>.
--------------------------------------------------------------------------------------------------------------------------------
function level_clicked(clicks) returns found:
    for place in clicks:
        picked = level_at(place)

        if hit = (picked != optional.none):
            return found = picked
        else miss:
            continue
    else no_clicks
    return found = optional.none


--------------------------------------------------------------------------------------------------------------------------------
Acts on each of the player's clicks in turn.

game: The game, which changes, as game.
selected: The square of the piece the player has selected, if there is one, which changes, as optional<value=square>.
clicks: The points clicked, in order, as list<element=point>.
--------------------------------------------------------------------------------------------------------------------------------
function act_on_clicks(mutable game, mutable selected, clicks):
    for place in clicks:
        clicked(mutable game, mutable selected, place)
    else no_clicks


--------------------------------------------------------------------------------------------------------------------------------
The move from one square to another, if the player to move may make it.

game: The game, as game.
from: The square the move starts from, if there is one, as optional<value=square>.
to: The square it ends on, if there is one, as optional<value=square>.

found: The move, or none, as optional<value=move>.
--------------------------------------------------------------------------------------------------------------------------------
function move_to(game, from, to) returns found:
    for candidate in legal_moves(game):
        if fits = ((optional.some(candidate.from) == from) and (optional.some(candidate.path.last()) == to)):
            return found = optional.some(candidate)
        else differs:
            continue
    else no_moves
    return found = optional.none


--------------------------------------------------------------------------------------------------------------------------------
Whether the player to move may move the piece on a square.

game: The game, as game.
square: The square, if there is one, as optional<value=square>.

can: Whether they may, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function may_move_from(game, square) returns can:
    for candidate in legal_moves(game):
        if movable = (optional.some(candidate.from) == square):
            return can = true
        else unmovable:
            continue
    else no_moves
    return can = false
