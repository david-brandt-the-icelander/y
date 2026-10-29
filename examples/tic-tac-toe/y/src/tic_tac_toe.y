module tic_tac_toe

from standard.console import console, console_error
from standard.wait import forever


-- The board, the marks on it, and how a game starts.
public facet setup:
    -- The board.
    constants board:
        -- How many squares the board has, numbered from 1 row by row from the top left.
        squares is uint32 = 9

        -- The board's eight lines of three squares: three rows, three columns and two diagonals.
        lines is list<list<uint32>> = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [1, 4, 7], [2, 5, 8], [3, 6, 9], [1, 5, 9], [3, 5, 7]]


    -- A mark a player puts in a square. The person plays crosses and moves first; the computer plays noughts.
    constants mark:
        cross = "X"
        nought = "O"


    -- One game.
    structure game:
        -- The squares, in the order they are numbered: the mark in each, or none.
        squares is list<optional<mark>>

        -- Whose turn it is.
        to_move is mark


    ----------------------------------------------------------------------------------------------------------------------------
    Starts a game.

    started: An empty board, with crosses to move, as game.
    ----------------------------------------------------------------------------------------------------------------------------
    function new_game() returns started:
        return started = game(squares: list.repeated(optional.none, times: board.squares), to_move: mark.cross)


-- Moves: reading them, making them, and choosing one.
public facet moves:
    -- Why what the player typed is not a move.
    constants move_error:
        not_a_number = "not a number"
        no_such_square = "no such square"
        taken = "the square is taken"


    ----------------------------------------------------------------------------------------------------------------------------
    Reads a move from what the player typed: the number of an empty square.

    game: The game, as game.
    line: What the player typed, such as `5`, as string.

    chosen: The square's number, as uint32.

    fails move_error.not_a_number: The line does not hold one whole number.
    fails move_error.no_such_square: The number is not that of a square.
    fails move_error.taken: The square already holds a mark.
    ----------------------------------------------------------------------------------------------------------------------------
    function read_move(game, line) returns chosen fails move_error:
        number = line.trim().parse_uint32()

        if not_a_number = (number == optional.none):
            fail move_error.not_a_number
        else a_number:
            conditional judged:
                condition judged.off_board = ((number == 0) or (number > board.squares)):
                    fail move_error.no_such_square
                condition judged.taken = (game.squares.at_number(number) != optional.none):
                    fail move_error.taken
                else judged.empty:
                    return chosen = number


    ----------------------------------------------------------------------------------------------------------------------------
    Puts the mark of the player to move in a square, and passes the turn to the other player.

    game: The game, which changes, as game.
    square: The square's number, as uint32.
    ----------------------------------------------------------------------------------------------------------------------------
    function place(mutable game, square)
            where unmarked = (game.squares.at_number(square) == optional.none):
        game.squares.set_number(square, optional.some(game.to_move))
        game.to_move := other(game.to_move)


    ----------------------------------------------------------------------------------------------------------------------------
    Chooses the computer's move: the first square, in the order they are numbered, whose value to the computer is the
    highest. With perfect play the computer never loses.

    game: The game, as game.

    chosen: The square's number, as uint32.
    ----------------------------------------------------------------------------------------------------------------------------
    function computer_move(game) returns chosen
            where still_playing = (not over(game)):
        open = open_squares(game)

        -- Lower than any square's value, so the first square looked at is taken.
        mutable best = -100
        mutable found = open[0]

        for square in open:
            mutable after = game
            place(mutable after, square)
            value = -value_of(after)

            if better = (value > best):
                best := value
                found := square
            else no_better:
                continue
        return chosen = found


--------------------------------------------------------------------------------------------------------------------------------
Whether a game has ended: a player has three in a row, or the board is full.

game: The game, as game.

ended: Whether it has, as boolean.
--------------------------------------------------------------------------------------------------------------------------------
function over(game) returns ended:
    return ended = ((winner(game) != optional.none) or (open_squares(game).count() == 0))


--------------------------------------------------------------------------------------------------------------------------------
The player who has three in a row, if one has.

game: The game, as game.

found: That player's mark, or none, as optional<mark>.
--------------------------------------------------------------------------------------------------------------------------------
function winner(game) returns found:
    for line in board.lines:
        first = game.squares.at_number(line[0])
        second = game.squares.at_number(line[1])
        third = game.squares.at_number(line[2])

        if three = ((first != optional.none) and (second == first) and (third == first)):
            return found = first
        else not_three:
            continue
    return found = optional.none


--------------------------------------------------------------------------------------------------------------------------------
The mark of the player who moves after the one with `player`.

player: A mark, as mark.

next: The other mark, as mark.
--------------------------------------------------------------------------------------------------------------------------------
function other(player) returns next:
    conditional current = player:
        condition current.cross = (current == mark.cross):
            return next = mark.nought
        condition current.nought = (current == mark.nought):
            return next = mark.cross


--------------------------------------------------------------------------------------------------------------------------------
The squares that hold no mark, in the order they are numbered.

game: The game, as game.

open: Their numbers, as list<uint32>.
--------------------------------------------------------------------------------------------------------------------------------
function open_squares(game) returns open:
    mutable found = list.new()

    for number in range(from: 1, through: board.squares):
        if marked = (game.squares.at_number(number) != optional.none):
            continue
        else empty:
            found.push(number)
    return open = found


--------------------------------------------------------------------------------------------------------------------------------
The value of a game to the player whose turn it is, when both players play perfectly from here on: more than zero for a
win, zero for a draw, and less than zero for a loss. A win is worth more the sooner it comes, and a loss costs more.

game: The game, as game.

value: The value, as int32.
--------------------------------------------------------------------------------------------------------------------------------
function value_of(game) returns value:
    open = open_squares(game)

    conditional now:
        -- The player who moved last made the line, so the player to move has lost.
        condition now.lost = (winner(game) != optional.none):
            return value = -1 - open.count().to_int32()
        condition now.drawn = (open.count() == 0):
            return value = 0
        else now.open:
            mutable best = -100

            for square in open:
                mutable after = game
                place(mutable after, square)
                best := best.max(-value_of(after))
            return value = best


--------------------------------------------------------------------------------------------------------------------------------
Draws the board, three rows of three squares. An empty square shows its number, so the player sees what to type.

game: The game, as game.

picture: The drawing, as string.
--------------------------------------------------------------------------------------------------------------------------------
function shown(game) returns picture:
    mutable rows = list.new()

    for first in [1, 4, 7]:
        mutable row = list.new()

        for number in range(from: first, through: first + 2):
            row.push(square_shown(game, number))
        line = row.join(separator: " | ")
        rows.push(" {line}")
    return picture = rows.join(separator: "\n---+---+---\n")


--------------------------------------------------------------------------------------------------------------------------------
How one square is drawn.

game: The game, as game.
number: The square's number, as uint32.

drawn: Its mark, or its number when it has none, as string.
--------------------------------------------------------------------------------------------------------------------------------
function square_shown(game, number) returns drawn:
    conditional held = game.squares.at_number(number):
        condition held.empty = (held == optional.none):
            return drawn = "{number}"
        else held.marked:
            return drawn = "{held}"


-- Playing a whole game at the console.
public facet console_play:
    -- Why a game could not be played to its end.
    type play_error:
        case input_closed
        case console(error is console_error)


    ----------------------------------------------------------------------------------------------------------------------------
    Plays one game at the console, the player's crosses against the computer's noughts, until a player has three in a
    row or the board is full. The player moves first.

    console: Where the game is shown and the moves are read, as console.
    forever: The right to wait for the player as long as they take, as forever.

    fails play_error.input_closed: The console closed before the game ended.
    fails play_error.console: The console could not be read or written.
    ----------------------------------------------------------------------------------------------------------------------------
    function play(injected console, injected forever) fails play_error:
        mutable game = new_game()
        console.write_line("Tic-tac-toe: you are X, and move first. Three in a row wins.") or fail

        while playing = (not over(game)):
            if persons_turn = (game.to_move == mark.cross):
                picture = shown(game)
                console.write_line(picture) or fail
                console.write("Your move (a square's number): ") or fail
                typed = console.read_line() forever because "a player may think for as long as they like" or fail

                if closed = (typed == optional.none):
                    fail play_error.input_closed
                else line_read:
                    conditional attempted = read_move(game, typed):
                        condition attempted.valid = (attempted == result.ok):
                            place(mutable game, attempted.value)
                        condition attempted.not_a_move = (attempted == result.err(move_error.not_a_number)):
                            console.write_line("Type the number of a square, from 1 to 9.") or fail
                        condition attempted.no_square = (attempted == result.err(move_error.no_such_square)):
                            console.write_line("There is no such square.") or fail
                        condition attempted.taken = (attempted == result.err(move_error.taken)):
                            console.write_line("That square is taken.") or fail
            else computers_turn:
                chosen = computer_move(game)
                place(mutable game, chosen)
                console.write_line("The computer plays {chosen}.") or fail
        else game_over:
            picture = shown(game)
            console.write_line(picture) or fail
            found = winner(game)

            if drawn = (found == optional.none):
                console.write_line("The board is full. A draw.") or fail
            else won:
                console.write_line("{found} has three in a row, and wins.") or fail
