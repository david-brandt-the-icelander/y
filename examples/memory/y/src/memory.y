module memory

from standard.http import http, http_error
from standard.random import random
from standard.time import clock, duration, expired, instant
from standard.wait import forever
from standard.window import colour, key, picture, picture_error, point, window


-- The board, the rules, the pictures, and how a game starts.
public facet setup:
    -- The board.
    constants board:
        -- How many cards are laid out across the board, and how many down it.
        columns is int32 = 5
        rows is int32 = 4

        -- How many cards there are, numbered from 1 row by row from the top left.
        cards is uint32 = 20

        -- How many pairs of cards there are: one for each picture.
        pairs is uint32 = 10


    -- The rules every game is played by, and how it is scored.
    constants game_rules:
        -- How many times the player may turn over two cards that do not match before the game is lost.
        lives is uint32 = 10

        -- How long two cards that do not match stay face up.
        show_time is duration = 1s

        -- How long the pictures may take to fetch.
        fetch_time is duration = 20s

        -- What each pair found scores.
        points_per_pair is uint32 = 100

        -- What each life left scores, when every pair is found.
        points_per_life is uint32 = 50

        -- What each whole second under the par time scores, when every pair is found.
        points_per_second is uint32 = 10

        -- The time to beat.
        par_time is duration = 120s


    -- The pictures the cards may show.
    constants faces:
        -- The OpenMoji codes of twenty animals' faces, from which each game chooses its pictures.
        codes is list<element_type=string> = [
            "1F436", "1F431", "1F42D", "1F439", "1F430", "1F98A", "1F43B", "1F43C", "1F428", "1F42F", "1F981", "1F42E",
            "1F437", "1F438", "1F435", "1F414", "1F427", "1F426", "1F989", "1F984",
        ]


    -- OpenMoji, the open-source emoji project, whose pictures the cards show. Its pictures are licensed CC BY-SA 4.0,
    -- and the game credits them.
    service openmoji:
        address = url"https://openmoji.org"


    -- Which side of a card is up.
    constants side:
        face_down = "face down"
        face_up = "face up"
        matched = "matched"


    -- Where a game stands.
    constants outcome:
        playing = "playing"
        won = "won"
        lost = "lost"


    -- One game.
    structure game:
        -- For each card, in the order they are numbered, the number of the picture it shows.
        pairs is list<element_type=uint32>

        -- For each card, in the same order, which side is up.
        sides is list<element_type=side>

        -- The pictures, numbered from 1.
        pictures is list<element_type=picture>

        -- The cards turned face up this turn, and not yet matched: none, one or two.
        turned is list<element_type=uint32>

        -- When the second card of the turn was turned, or when the game started.
        turned_at is instant

        -- How many pairs have been found.
        matches is uint32

        -- How many lives are left.
        lives is uint32

        -- When the game started.
        started is instant

        -- How long the game has been played, which stops when it ends.
        elapsed is duration


    -- Why a game could not be set up.
    type setup_error:
        case unreachable(error is http_error)
        case timed_out(deadline is expired)
        case not_a_picture(error is picture_error)


    ------------------------------------------------------------------------------------------------------------------------------------
    Starts a game: chooses the pictures at random, fetches them from OpenMoji, and deals two cards of each, face down,
    in an order drawn at random.

    started: The game, as game.

    random: What chooses the pictures and deals the cards, as random.
    clock: What tells the time the game starts, as clock.
    site: The OpenMoji website, as http<service=openmoji>.

    fails setup_error.unreachable: OpenMoji did not give a picture.
    fails setup_error.timed_out: The pictures took longer than the fetch time to come.
    fails setup_error.not_a_picture: What OpenMoji gave is not a picture.
    ------------------------------------------------------------------------------------------------------------------------------------
    function new_game(injected random, injected clock, injected site) returns started fails setup_error
            ensures playing = (standing(started) == outcome.playing)
            ensures nothing_turned = started.turned.is_empty():
        codes = random.chosen(from: faces.codes, count: board.pairs)
        mutable pictures = list.new()

        region fetch is within game_rules.fetch_time or fail:
            for code in codes:
                drawing = site.get(path"/data/color/svg/{code}.svg") or fail
                made = picture.from_svg(drawing, size: layout.picture_size) or fail
                pictures.push(made)
        mutable dealt = list.new()

        for number in range(from: 1, through: board.pairs):
            dealt.push(number)
            dealt.push(number)
        now = clock.now()
        return started = game(
            pairs: random.shuffled(dealt),
            sides: list.repeated(side.face_down, times: board.cards),
            pictures: pictures,
            turned: [],
            turned_at: now,
            matches: 0,
            lives: game_rules.lives,
            started: now,
            elapsed: 0s,
        )


-- Turning cards over, and how a game ends and is scored.
public facet rules:
    ------------------------------------------------------------------------------------------------------------------------------------
    The card the player may turn over by clicking a point, if there is one.

    game: The game, as game.
    place: The point clicked, as point.

    choice: The card's number, or none when the point is not on a card, the card is face up, or two cards are already
      face up, as optional<value_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------------------
    function choosable(game, place) returns choice:
        number = card_at(place)

        conditional judged:
            condition judged.off_cards = (number == optional.none):
                return choice = optional.none
            condition judged.waiting = (game.turned.count() == 2):
                return choice = optional.none
            condition judged.showing = (game.sides.at_number(number) != side.face_down):
                return choice = optional.none
            else judged.free:
                return choice = number


    ------------------------------------------------------------------------------------------------------------------------------------
    Turns a card face up. The second card of a turn either matches the first, and the pair stays face up, or it does
    not, and costs a life.

    game: The game, which changes, as game.
    number: The card's number, as uint32.
    now: The time now, as instant.
    ------------------------------------------------------------------------------------------------------------------------------------
    function turn_over(mutable game, number, now)
            where may_turn = ((game.sides.at_number(number) == side.face_down) and (game.turned.count() < 2)):
        conditional turn:
            condition turn.first = game.turned.is_empty():
                game.sides.set_number(number, side.face_up)
                game.turned.push(number)
            condition turn.matching = (game.pairs.at_number(game.turned[0]) == game.pairs.at_number(number)):
                game.sides.set_number(game.turned[0], side.matched)
                game.sides.set_number(number, side.matched)
                game.turned := []
                game.matches := game.matches + 1
            else turn.different:
                game.sides.set_number(number, side.face_up)
                game.turned.push(number)
                game.turned_at := now
                game.lives := game.lives - 1


    ------------------------------------------------------------------------------------------------------------------------------------
    Turns two cards that did not match face down again.

    game: The game, which changes, as game.
    ------------------------------------------------------------------------------------------------------------------------------------
    function turn_back(mutable game)
            where two_turned = (game.turned.count() == 2):
        for number in game.turned:
            game.sides.set_number(number, side.face_down)
        game.turned := []


    ------------------------------------------------------------------------------------------------------------------------------------
    Where a game stands: won when every pair is found, lost when no life is left, and otherwise still being played.

    game: The game, as game.

    reached: Where it stands, as outcome.
    ------------------------------------------------------------------------------------------------------------------------------------
    function standing(game) returns reached:
        conditional judged:
            condition judged.won = (game.matches == board.pairs):
                return reached = outcome.won
            condition judged.lost = (game.lives == 0):
                return reached = outcome.lost
            else judged.playing:
                return reached = outcome.playing


    ------------------------------------------------------------------------------------------------------------------------------------
    The score of a game: points for each pair found and, when every pair is found, for each life left and each whole
    second under the par time.

    game: The game, as game.

    points: The score, as uint32.
    ------------------------------------------------------------------------------------------------------------------------------------
    function score(game) returns points:
        found = game.matches * game_rules.points_per_pair

        if won = (standing(game) == outcome.won):
            return points = found + game.lives * game_rules.points_per_life + time_bonus(game.elapsed)
        else unfinished:
            return points = found


----------------------------------------------------------------------------------------------------------------------------------------
The points for finishing a game under the par time.

elapsed: How long the game took, as duration.

bonus: The points for each whole second under the par time, or none when it took longer, as uint32.
----------------------------------------------------------------------------------------------------------------------------------------
function time_bonus(elapsed) returns bonus:
    if quick = (elapsed < game_rules.par_time):
        return bonus = (game_rules.par_time - elapsed).whole_seconds() * game_rules.points_per_second
    else slow:
        return bonus = 0


----------------------------------------------------------------------------------------------------------------------------------------
The card under a point, if there is one.

place: The point, as point.

found: The card's number, or none when the point is between the cards or off the board, as optional<value_type=uint32>.
----------------------------------------------------------------------------------------------------------------------------------------
function card_at(place) returns found:
    mutable number = 0

    for row in range(from: 0, below: board.rows):
        for column in range(from: 0, below: board.columns):
            number := number + 1
            corner = corner_of(row, column)
            across = ((place.x >= corner.x) and (place.x < corner.x + layout.card_size))
            down = ((place.y >= corner.y) and (place.y < corner.y + layout.card_size))

            if inside = (across and down):
                return found = optional.some(number)
            else outside:
                continue
    return found = optional.none


----------------------------------------------------------------------------------------------------------------------------------------
The top left corner of the card in a row and a column, each counted from 0.

row: The row, as int32.
column: The column, as int32.

corner: The corner, as point.
----------------------------------------------------------------------------------------------------------------------------------------
function corner_of(row, column) returns corner:
    return corner = point(x: layout.gap + column * layout.step, y: layout.gap + row * layout.step)


-- What the player sees of a game.
public facet drawing:
    -- The sizes of what is drawn, in pixels.
    constants layout:
        -- The side of a card.
        card_size is int32 = 112

        -- The space between two cards, and around them.
        gap is int32 = 12

        -- From one card to the next.
        step is int32 = layout.card_size + layout.gap

        -- How far a picture is drawn in from its card's edge.
        picture_inset is int32 = 8

        -- The side of a picture: the card's, less the inset on both sides.
        picture_size is uint16 = 96

        -- The height of the bar beneath the board, where the score is written.
        bar_height is int32 = 84

        -- The height of the text.
        text_size is uint32 = 20
        width is int32 = layout.gap + board.columns * layout.step
        height is int32 = layout.gap + board.rows * layout.step + layout.bar_height


    -- The colours of what is drawn.
    constants palette:
        background is colour = colour(red: 24, green: 26, blue: 33)
        back is colour = colour(red: 70, green: 90, blue: 140)
        face is colour = colour(red: 235, green: 235, blue: 225)
        matched is colour = colour(red: 170, green: 215, blue: 160)
        text is colour = colour(red: 230, green: 230, blue: 230)
        credit is colour = colour(red: 140, green: 140, blue: 150)


    ------------------------------------------------------------------------------------------------------------------------------------
    The lines written beneath the board: how the game stands, and what the player can do.

    game: The game, as game.
    highest: The best score so far, as uint32.

    lines: The two lines, as list<element_type=string>.
    ------------------------------------------------------------------------------------------------------------------------------------
    function banner(game, highest) returns lines:
        seconds = game.elapsed.whole_seconds()

        conditional shown = standing(game):
            condition shown.playing = (shown == outcome.playing):
                return lines = [
                    "Pairs {game.matches} of {board.pairs}   Lives {game.lives}   Time {seconds}s   Best {highest}",
                    "Click two cards to turn them over. Escape quits.",
                ]
            condition shown.won = (shown == outcome.won):
                return lines = [
                    "All pairs in {seconds}s! Score {score(game)}   Best {highest}",
                    "Space plays again. Escape quits.",
                ]
            condition shown.lost = (shown == outcome.lost):
                return lines = [
                    "Out of lives with {game.matches} pairs. Score {score(game)}   Best {highest}",
                    "Space plays again. Escape quits.",
                ]


    ------------------------------------------------------------------------------------------------------------------------------------
    Draws a game: the cards, and the banner and the pictures' credit beneath.

    game: The game, as game.
    highest: The best score so far, as uint32.

    window: Where the game is drawn, as window.
    ------------------------------------------------------------------------------------------------------------------------------------
    function draw(game, highest, injected window):
        window.clear(palette.background)
        mutable number = 0

        for row in range(from: 0, below: board.rows):
            for column in range(from: 0, below: board.columns):
                number := number + 1
                draw_card(game, number, corner_of(row, column))
        lines = banner(game, highest)
        top = layout.height - layout.bar_height
        credit = "Pictures: OpenMoji, CC BY-SA 4.0"
        window.write(text: lines[0], x: layout.gap, y: top + 24, size: layout.text_size, colour: palette.text)
        window.write(text: lines[1], x: layout.gap, y: top + 50, size: layout.text_size, colour: palette.text)
        window.write(text: credit, x: layout.gap, y: top + 76, size: layout.text_size, colour: palette.credit)


----------------------------------------------------------------------------------------------------------------------------------------
Draws one card: its back, or its face with its picture.

game: The game, as game.
number: The card's number, as uint32.
corner: The card's top left corner, as point.

window: Where the card is drawn, as window.
----------------------------------------------------------------------------------------------------------------------------------------
function draw_card(game, number, corner, injected window):
    conditional shown = game.sides.at_number(number):
        condition shown.face_down = (shown == side.face_down):
            fill_card(corner, palette.back)
        condition shown.face_up = (shown == side.face_up):
            fill_card(corner, palette.face)
            draw_picture(game, number, corner)
        condition shown.matched = (shown == side.matched):
            fill_card(corner, palette.matched)
            draw_picture(game, number, corner)


----------------------------------------------------------------------------------------------------------------------------------------
Fills the square of one card.

corner: The card's top left corner, as point.
shade: The colour, as colour.

window: Where it is drawn, as window.
----------------------------------------------------------------------------------------------------------------------------------------
function fill_card(corner, shade, injected window):
    window.fill(x: corner.x, y: corner.y, width: layout.card_size, height: layout.card_size, colour: shade)


----------------------------------------------------------------------------------------------------------------------------------------
Draws a card's picture.

game: The game, as game.
number: The card's number, as uint32.
corner: The card's top left corner, as point.

window: Where the picture is drawn, as window.
----------------------------------------------------------------------------------------------------------------------------------------
function draw_picture(game, number, corner, injected window):
    picture = game.pictures.at_number(game.pairs.at_number(number))
    window.picture(picture, x: corner.x + layout.picture_inset, y: corner.y + layout.picture_inset)


----------------------------------------------------------------------------------------------------------------------------------------
Shows that the pictures are being fetched, which takes a moment, while the window would otherwise stand still.

window: Where the message is shown, as window.
forever: The right to wait for the next frame as long as it takes, as forever.
----------------------------------------------------------------------------------------------------------------------------------------
function show_fetching(injected window, injected forever):
    window.clear(palette.background)
    message = "Fetching pictures from OpenMoji..."
    window.write(text: message, x: layout.gap, y: layout.height / 2, size: layout.text_size, colour: palette.text)

    region frame_shown is forever because "a hidden window shows no frames, and the game waits until it is shown":
        window.next_frame()


-- Playing in a window, one game after another, until the window is closed.
public facet window_play:
    ------------------------------------------------------------------------------------------------------------------------------------
    Plays Memory in a window: one game after another, until the player closes the window or presses Escape. When a
    game ends, Space starts the next, with new pictures.

    best: The best score of the games played, as uint32.

    window: Where the games are drawn and the clicks and keys are read, as window.
    clock: What times the games, as clock.
    random: What chooses the pictures and deals the cards, as random.
    site: The OpenMoji website, as http<service=openmoji>.
    forever: The right to wait for the next frame as long as it takes, as forever.

    fails setup_error.unreachable: OpenMoji did not give a picture.
    fails setup_error.timed_out: The pictures took longer than the fetch time to come.
    fails setup_error.not_a_picture: What OpenMoji gave is not a picture.
    ------------------------------------------------------------------------------------------------------------------------------------
    function play(injected window, injected clock, injected random, injected site, injected forever) returns best fails setup_error:
        show_fetching()
        mutable game = new_game() or fail
        mutable highest = 0
        mutable quitting = false

        while open = ((not quitting) and (not window.close_requested())):
            keys = window.keys_pressed()
            now = clock.now()
            quitting := keys.contains(key.escape)
            reached = standing(game)

            conditional moment:
                condition moment.again = ((reached != outcome.playing) and keys.contains(key.space)):
                    show_fetching()
                    game := new_game() or fail
                condition moment.hiding = ((game.turned.count() == 2) and (now - game.turned_at >= game_rules.show_time)):
                    turn_back(mutable game)
                condition moment.picking = (reached == outcome.playing):
                    game.elapsed := now - game.started
                    pick(mutable game, window.clicks(), now)
                else moment.over:
                    highest := highest.max(score(game))
            draw(game, highest)

            region frame_shown is forever because "a hidden window shows no frames, and the game waits until it is shown":
                window.next_frame()
        else closed_at_once
        return best = highest


----------------------------------------------------------------------------------------------------------------------------------------
Turns over the cards the player clicked, in the order clicked. A click that turns no card is ignored.

game: The game, which changes, as game.
clicks: The points clicked, as list<element_type=point>.
now: The time now, as instant.
----------------------------------------------------------------------------------------------------------------------------------------
function pick(mutable game, clicks, now):
    for place in clicks:
        choice = choosable(game, place)

        if chosen = (choice != optional.none):
            turn_over(mutable game, choice, now)
        else ignored:
            continue
    else no_clicks
