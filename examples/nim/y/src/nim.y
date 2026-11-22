module nim

from standard.console import console, console_error
from standard.wait import forever


-- A game, and how it starts.
public facet setup:
    -- The rules every game is played by.
    constants game_rules:
        -- How many counters each heap holds at the start, from the first heap to the last.
        starting_heaps is list<element_type=uint32> = [3, 4, 5]


    -- One game: how many counters are left in each heap.
    structure game:
        heaps is list<element_type=uint32>


    -- Who is playing.
    constants player:
        person = "you"
        computer = "the computer"


    ------------------------------------------------------------------------------------------------------------------------
    Starts a game.

    started: A game with every heap full, as game.
    ------------------------------------------------------------------------------------------------------------------------
    function new_game() returns started:
        return started = game(heaps: game_rules.starting_heaps)


-- Moves: reading them, checking them, making them, and choosing one.
public facet moves:
    -- A move: which heap, numbered from 1 as the player sees it, and how many counters to take from it.
    structure move:
        heap is uint32
        take is uint32


    -- Why what the player typed is not a move.
    constants move_error:
        not_two_numbers = "not two numbers"
        no_such_heap = "no such heap"
        nothing_taken = "nothing taken"
        too_many = "more than the heap holds"


    ------------------------------------------------------------------------------------------------------------------------
    Reads a move from what the player typed: the heap's number, then how many counters to take.

    line: What the player typed, such as `2 3`, as string.

    parsed: The move, not yet checked against a game, as move.

    fails move_error.not_two_numbers: The line does not hold exactly two whole numbers.
    ------------------------------------------------------------------------------------------------------------------------
    function parse_move(line) returns parsed fails move_error:
        pieces = line.split_whitespace()

        if not_two = (pieces.count() != 2):
            fail move_error.not_two_numbers
        else two:
            heap = pieces[0].parse_uint32()
            take = pieces[1].parse_uint32()

            if not_numbers = ((heap == optional.none) or (take == optional.none)):
                fail move_error.not_two_numbers
            else numbers:
                return parsed = move(heap: heap, take: take)


    ------------------------------------------------------------------------------------------------------------------------
    Checks that a move can be made in a game.

    game: The game, as game.
    chosen: The move, as move.

    allowed: The same move, as move.

    fails move_error.no_such_heap: The game has no heap with that number.
    fails move_error.nothing_taken: The move takes no counters.
    fails move_error.too_many: The heap holds fewer counters than the move takes.
    ------------------------------------------------------------------------------------------------------------------------
    function check(game, chosen) returns allowed fails move_error:
        if no_such_heap = ((chosen.heap == 0) or (chosen.heap > game.heaps.count().to_uint32())):
            fail move_error.no_such_heap
        else heap_exists:
            heap = game.heaps.at_number(chosen.heap)

            conditional judged:
                condition judged.nothing_taken = (chosen.take == 0):
                    fail move_error.nothing_taken
                condition judged.too_many = (chosen.take > heap):
                    fail move_error.too_many
                else judged.fits:
                    return allowed = chosen


    ------------------------------------------------------------------------------------------------------------------------
    Reads a move from what the player typed, and checks it against the game.

    game: The game, as game.
    line: What the player typed, as string.

    chosen: The move, which can be made, as move.

    fails move_error.not_two_numbers: The line does not hold exactly two whole numbers.
    fails move_error.no_such_heap: The game has no heap with that number.
    fails move_error.nothing_taken: The move takes no counters.
    fails move_error.too_many: The heap holds fewer counters than the move takes.
    ------------------------------------------------------------------------------------------------------------------------
    function read_move(game, line) returns chosen fails move_error:
        parsed = parse_move(line) or fail
        return chosen = check(game, parsed) or fail


    ------------------------------------------------------------------------------------------------------------------------
    Makes a move.

    game: The game, which loses the counters taken, as game.
    chosen: The move, as move.
    ------------------------------------------------------------------------------------------------------------------------
    function remove(mutable game, chosen)
            where allowed = (check(game, chosen) == result.ok):
        heap = game.heaps.at_number(chosen.heap)
        game.heaps.set_number(chosen.heap, heap - chosen.take)


    ------------------------------------------------------------------------------------------------------------------------
    Chooses the computer's move. It leaves a nim-sum of zero when it can, which wins every game that can be won.

    game: The game, as game.

    chosen: The move, as move.
    ------------------------------------------------------------------------------------------------------------------------
    function computer_move(game) returns chosen
            where still_playing = (counters_left(game) > 0):
        sum = nim_sum(game.heaps)

        if lost = (sum == 0):
            -- Every move leaves a nim-sum other than zero: take one counter from the largest heap, and hope.
            largest = game.heaps.largest()
            return chosen = move(heap: game.heaps.number_of(largest), take: 1)
        else winnable:
            -- Some heap, made smaller, leaves a nim-sum of zero.
            for number in range(from: 1, through: game.heaps.count().to_uint32()):
                heap = game.heaps.at_number(number)
                target = heap.xor(sum)

                if winning = (target < heap):
                    return chosen = move(heap: number, take: heap - target)
                else not_this_heap:
                    continue
            else no_heaps:
                fail not_reachable
            fail not_reachable


----------------------------------------------------------------------------------------------------------------------------
How many counters are left in the game.

game: The game, as game.

left: The counters in every heap together, as uint32.
----------------------------------------------------------------------------------------------------------------------------
function counters_left(game) returns left:
    mutable total = 0

    for heap in game.heaps:
        total := total + heap
    else no_heaps
    return left = total


----------------------------------------------------------------------------------------------------------------------------
The nim-sum of the heaps: their sizes combined with exclusive or.

heaps: The heaps, as list<element_type=uint32>.

sum: The nim-sum, which is zero exactly when the player to move loses against perfect play, as uint32.
----------------------------------------------------------------------------------------------------------------------------
function nim_sum(heaps) returns sum:
    mutable total = 0

    for heap in heaps:
        total := total.xor(heap)
    else no_heaps
    return sum = total


----------------------------------------------------------------------------------------------------------------------------
Draws the heaps, one line each, with a bar for each counter.

game: The game, as game.

picture: The drawing, as string.
----------------------------------------------------------------------------------------------------------------------------
function shown(game) returns picture:
    mutable lines = list.new()
    mutable number = 0

    for heap in game.heaps:
        number := number + 1
        counters = "|".repeated(heap)
        lines.push("  {number}: {counters}")
    else no_heaps
    return picture = lines.join(separator: "\n")


-- Playing a whole game at the console.
public facet console_play:
    -- Why a game could not be played to its end.
    type play_error:
        case input_closed
        case console(error is console_error)


    ------------------------------------------------------------------------------------------------------------------------
    Plays a game at the console, the player against the computer, until the last counter is taken. The player moves
    first.

    started: The game as it starts, as game.

    console: Where the game is shown and the moves are read, as console.
    forever: The right to wait for the player as long as they take, as forever.

    fails play_error.input_closed: The console closed before the game ended.
    fails play_error.console: The console could not be read or written.
    ------------------------------------------------------------------------------------------------------------------------
    function play(started, injected console, injected forever) fails play_error:
        mutable game = started
        mutable turn = player.person
        console.write_line("Nim: take any number of counters from one heap. Whoever takes the last counter wins.") or fail

        while playing = (counters_left(game) > 0):
            picture = shown(game)
            console.write_line(picture) or fail

            if persons_turn = (turn == player.person):
                turn := persons_move(mutable game) or fail
            else computers_turn:
                chosen = computer_move(game)
                remove(mutable game, chosen)
                console.write_line("The computer takes {chosen.take} from heap {chosen.heap}.") or fail
                turn := player.person
        else nothing_to_play
        announce(turn) or fail


----------------------------------------------------------------------------------------------------------------------------
Reads the player's move and makes it, or says what is wrong with what they typed.

game: The game, which loses the counters taken when the move is made, as game.

next: Who moves next: the computer once a move is made, and the player again otherwise, as player.

console: Where the move is read, and what is wrong with it is shown, as console.
forever: The right to wait for the player as long as they take, as forever.

fails play_error.input_closed: The console closed before the player moved.
fails play_error.console: The console could not be read or written.
----------------------------------------------------------------------------------------------------------------------------
function persons_move(mutable game, injected console, injected forever) returns next fails play_error:
    console.write("Your move (heap, then counters): ") or fail
    typed = console.read_line() forever because "a player may think for as long as they like" or fail

    if closed = (typed == optional.none):
        fail play_error.input_closed
    else line_read:
        conditional attempted = read_move(game, typed):
            condition attempted.valid = (attempted == result.ok):
                remove(mutable game, attempted.value)
                return next = player.computer
            else attempted.invalid:
                console.write_line(complaint(attempted.failure())) or fail
                return next = player.person


----------------------------------------------------------------------------------------------------------------------------
What the game says to what is not a move.

error: Why it is not, as move_error.

said: The line to write, as string.
----------------------------------------------------------------------------------------------------------------------------
function complaint(error) returns said:
    conditional wrong = error:
        condition wrong.not_a_move = (wrong == move_error.not_two_numbers):
            return said = "Type a heap's number and how many to take, such as 2 3."
        condition wrong.no_heap = (wrong == move_error.no_such_heap):
            return said = "There is no such heap."
        condition wrong.none_taken = (wrong == move_error.nothing_taken):
            return said = "Take at least one counter."
        condition wrong.too_many = (wrong == move_error.too_many):
            return said = "That heap does not hold that many."


----------------------------------------------------------------------------------------------------------------------------
Says who won.

turn: Whose turn it is once the last counter is taken: the other player's than the winner's, as player.

console: Where it is said, as console.

fails console_error: The console could not be written.
----------------------------------------------------------------------------------------------------------------------------
function announce(turn, injected console) fails console_error:
    if computer_won = (turn == player.person):
        console.write_line("The computer took the last counter. The computer wins.") or fail
    else person_won:
        console.write_line("You took the last counter. You win!") or fail
