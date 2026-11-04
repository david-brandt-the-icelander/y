-- What stands in for the pictures in the tests.
constants stand_in:
    -- A drawing that a fake site gives for every picture: a red circle.
    drawing = "<svg viewBox='0 0 72 72' xmlns='http://www.w3.org/2000/svg'><circle cx='36' cy='36' r='30' fill='red'/></svg>"

    -- The order of the cards for the rules: each picture's two cards side by side.
    side_by_side is list<element_type=uint32> = [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10]


--------------------------------------------------------------------------------------------------------------------------------
A game dealt with its cards in a given order, all face down, with no pictures: for the rules, which draw nothing.

pairs: For each card, the number of the picture it shows, as list<element_type=uint32>.
at: When the game started, as instant.

made: The game, as game.
--------------------------------------------------------------------------------------------------------------------------------
function dealt(pairs, at) returns made:
    return made = game(
        pairs: pairs,
        sides: list.repeated(side.face_down, times: board.cards),
        pictures: [],
        turned: [],
        turned_at: at,
        matches: 0,
        lives: game_rules.lives,
        started: at,
        elapsed: 0s,
    )


--------------------------------------------------------------------------------------------------------------------------------
A frame of a fake window in which the player clicks the middle of a card.

row: The card's row, counted from 0, as int32.
column: The card's column, counted from 0, as int32.

made: The frame, as frame.
--------------------------------------------------------------------------------------------------------------------------------
function clicking(row, column) returns made:
    corner = corner_of(row, column)
    half = layout.card_size / 2
    return made = frame(clicks: [point(x: corner.x + half, y: corner.y + half)])


--------------------------------------------------------------------------------------------------------------------------------
The frames of a fake window in which the player turns over every pair, with the seed 7: the first card of each pair is
clicked, then the second.

made: The frames, as list<element_type=frame>.
--------------------------------------------------------------------------------------------------------------------------------
function every_pair_clicked() returns made:
    return made = [
        frame(),
        clicking(row: 3, column: 1), clicking(row: 3, column: 2),
        clicking(row: 0, column: 0), clicking(row: 2, column: 3),
        clicking(row: 1, column: 1), clicking(row: 3, column: 3),
        clicking(row: 0, column: 3), clicking(row: 3, column: 4),
        clicking(row: 1, column: 0), clicking(row: 2, column: 1),
        clicking(row: 2, column: 0), clicking(row: 2, column: 2),
        clicking(row: 1, column: 3), clicking(row: 2, column: 4),
        clicking(row: 0, column: 4), clicking(row: 3, column: 0),
        clicking(row: 1, column: 2), clicking(row: 1, column: 4),
        clicking(row: 0, column: 1), clicking(row: 0, column: 2),
    ]


-- A click between the cards chooses no card.
test a_click_between_the_cards_chooses_nothing
        covers choosable > judged.off_cards:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        choice = choosable(game, point(x: 6, y: 6))
    then:
        nothing = (choice == optional.none)


-- No card can be chosen while two that do not match are face up.
test no_third_card_can_be_chosen_while_two_show
        covers choosable > judged.waiting:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 3, clock.now())
    when:
        choice = choosable(game, point(x: 564, y: 68))
    then:
        no_third_card = (choice == optional.none)


-- A card turned face up cannot be chosen again.
test a_face_up_card_cannot_be_chosen_again
        covers choosable > judged.showing:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        turn_over(mutable game, 2, clock.now())
    when:
        choice = choosable(game, point(x: 192, y: 68))
    then:
        not_again = (choice == optional.none)


-- A click on a card that is face down chooses it.
test a_click_on_a_card_chooses_it
        covers choosable > judged.free:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        choice = choosable(game, point(x: 192, y: 68))
    then:
        the_second_card = (choice == optional.some(2))


-- The first card of a turn turns face up, and waits for the second.
test the_first_card_of_a_turn_turns_face_up
        covers turn_over > turn.first:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        turn_over(mutable game, 1, clock.now())
    then:
        face_up = (game.sides.at_number(1) == side.face_up)
        waiting_for_the_second = (game.turned == [1])
        no_life_lost = (game.lives == game_rules.lives)


-- Two cards with the same picture stay face up, and cost no life.
test a_matching_pair_stays_face_up
        covers turn_over > turn.matching:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        turn_over(mutable game, 1, clock.now())
    when:
        turn_over(mutable game, 2, clock.now())
    then:
        first_matched = (game.sides.at_number(1) == side.matched)
        second_matched = (game.sides.at_number(2) == side.matched)
        one_pair = (game.matches == 1)
        a_new_turn = game.turned.is_empty()
        no_life_lost = (game.lives == game_rules.lives)


-- Two cards with different pictures both show, and cost a life.
test a_pair_that_does_not_match_costs_a_life
        covers turn_over > turn.different:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        turn_over(mutable game, 1, clock.now())
    when:
        turn_over(mutable game, 3, clock.now())
    then:
        second_face_up = (game.sides.at_number(3) == side.face_up)
        both_showing = (game.turned == [1, 3])
        no_pair = (game.matches == 0)
        a_life_lost = (game.lives == game_rules.lives - 1)


-- Two cards that do not match turn face down again.
test two_cards_that_do_not_match_turn_back
        covers turn_back > number'many:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 3, clock.now())
    when:
        turn_back(mutable game)
    then:
        first_face_down = (game.sides.at_number(1) == side.face_down)
        second_face_down = (game.sides.at_number(3) == side.face_down)
        a_new_turn = game.turned.is_empty()


-- Finding every pair wins the game.
test finding_every_pair_wins
        covers standing > judged.won:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := board.pairs
    when:
        reached = standing(game)
    then:
        won = (reached == outcome.won)


-- Losing every life loses the game.
test losing_every_life_loses
        covers standing > judged.lost:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.lives := 0
    when:
        reached = standing(game)
    then:
        lost = (reached == outcome.lost)


-- A game just dealt is being played.
test a_new_game_is_being_played
        covers standing > judged.playing:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        reached = standing(game)
    then:
        playing = (reached == outcome.playing)


-- A game won scores its pairs, its lives left and the seconds under par.
test a_won_game_scores_lives_and_seconds
        covers score > won:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := board.pairs
        game.lives := 4
        game.elapsed := 100s
    when:
        points = score(game)
    then:
        -- Expected 1000 for the pairs, 200 for the lives and 200 for the seconds.
        pairs_lives_and_seconds = (points == 1400)


-- A game not won scores only its pairs.
test an_unfinished_game_scores_its_pairs
        covers score > unfinished:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := 3
        game.lives := 0
    when:
        points = score(game)
    then:
        three_pairs = (points == 300)


-- A game finished under the par time scores each whole second under it.
test a_game_under_par_scores_its_seconds
        covers time_bonus > quick:
    when:
        bonus = time_bonus(100s)
    then:
        twenty_seconds = (bonus == 200)


-- A game finished in exactly the par time scores nothing for the time.
test a_game_exactly_on_par_scores_no_seconds
        covers time_bonus > slow'eq:
    when:
        bonus = time_bonus(game_rules.par_time)
    then:
        nothing = (bonus == 0)


-- A game finished over the par time scores nothing for the time.
test a_game_over_par_scores_no_seconds
        covers time_bonus > slow'gt:
    when:
        bonus = time_bonus(150s)
    then:
        nothing = (bonus == 0)


-- A point on the first card is on card 1.
test the_first_card_is_found
        covers card_at > row > column > inside:
    when:
        found = card_at(point(x: 68, y: 68))
    then:
        card_one = (found == optional.some(1))


-- A point on a later card of the top row is on that card.
test a_card_along_the_top_row_is_found
        covers card_at > row > column'many > inside:
    when:
        found = card_at(point(x: 192, y: 68))
    then:
        card_two = (found == optional.some(2))


-- A point on the first card of a lower row is on that card.
test a_card_down_the_first_column_is_found
        covers card_at > row'many > column > inside:
    when:
        found = card_at(point(x: 68, y: 192))
    then:
        card_six = (found == optional.some(6))


-- A point on a card neither in the top row nor in the first column is on that card.
test a_card_down_and_along_is_found
        covers card_at > row'many > column'many > inside:
    when:
        found = card_at(point(x: 192, y: 192))
    then:
        card_seven = (found == optional.some(7))


-- A point between two columns of cards is on no card.
test a_click_between_two_columns_finds_nothing
        covers card_at > row'many > column'many > outside'1:
    when:
        found = card_at(point(x: 130, y: 68))
    then:
        nothing = (found == optional.none)


-- A point below a card, in the gap above the next row, is on no card.
test a_click_between_two_rows_finds_nothing
        covers card_at > row'many > column'many > outside'2:
    when:
        found = card_at(point(x: 68, y: 130))
    then:
        nothing = (found == optional.none)


-- A card's corner is a gap in from the edge, and a step further for each row and column before it.
test a_card_corner_is_a_step_per_row_and_column
        covers corner_of:
    when:
        corner = corner_of(1, 2)
    then:
        across = (corner.x == 260)
        down = (corner.y == 136)


-- A frame with no clicks turns no card.
test no_clicks_turn_nothing
        covers pick > no_clicks:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [], clock.now())
    then:
        nothing_turned = game.turned.is_empty()


-- A click on a card turns it over.
test a_click_on_a_card_turns_it
        covers pick > place > chosen:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [point(x: 68, y: 68)], clock.now())
    then:
        first_up = (game.turned == [1])


-- A click between the cards is ignored.
test a_click_between_the_cards_is_ignored
        covers pick > place > ignored:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [point(x: 6, y: 6)], clock.now())
    then:
        nothing_turned = game.turned.is_empty()


-- Two clicks in one frame turn over two cards, in the order clicked.
test two_clicks_in_a_frame_turn_two_cards
        covers pick > place'many > chosen:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [point(x: 68, y: 68), point(x: 192, y: 68)], clock.now())
    then:
        a_pair = (game.matches == 1)


-- Two clicks in one frame, both between the cards, are both ignored.
test two_clicks_between_the_cards_are_ignored
        covers pick > place'many > ignored:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [point(x: 6, y: 6), point(x: 130, y: 68)], clock.now())
    then:
        nothing_turned = game.turned.is_empty()


-- The banner of a game being played says its pairs, lives, time and the best score, and how to play.
test the_banner_says_how_a_game_is_going
        covers banner > shown.playing:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        lines = banner(game, 50)
    then:
        says_playing = (lines[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50")
        says_how = (lines[1] == "Click two cards to turn them over. Escape quits.")


-- The banner of a game won says its time and its score, and that Space plays again.
test the_banner_says_a_game_is_won
        covers banner > shown.won:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := board.pairs
        game.lives := 4
        game.elapsed := 100s
    when:
        lines = banner(game, 1400)
    then:
        says_won = (lines[0] == "All pairs in 100s! Score 1400   Best 1400")
        says_again = (lines[1] == "Space plays again. Escape quits.")


-- The banner of a game lost says the pairs found and its score, and that Space plays again.
test the_banner_says_a_game_is_lost
        covers banner > shown.lost:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := 3
        game.lives := 0
    when:
        lines = banner(game, 50)
    then:
        says_lost = (lines[0] == "Out of lives with 3 pairs. Score 300   Best 50")
        says_again = (lines[1] == "Space plays again. Escape quits.")


-- Drawing a game draws every card, and writes the banner and the pictures' credit beneath.
test a_game_is_drawn_with_its_banner_and_credit
        covers draw > row'many > column'many:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        draw(game, 50)
    then:
        banner_written = (window.written()[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50")
        credited = (window.written().last() == optional.some("Pictures: OpenMoji, CC BY-SA 4.0"))
        all_face_down = (window.pictures_drawn() == 0)


-- A card face down shows its back, and no picture.
test a_face_down_card_shows_no_picture
        covers draw_card > shown.face_down:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        draw_card(game, 1, corner_of(0, 0))
    then:
        no_picture = (window.pictures_drawn() == 0)


-- A card face up shows its picture.
test a_face_up_card_shows_its_picture
        covers draw_card > shown.face_up:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 1s)
    given:
        made = picture.from_svg(stand_in.drawing, size: layout.picture_size) or fail
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.pictures := [made]
        turn_over(mutable game, 1, clock.now())
    when:
        draw_card(game, 1, corner_of(0, 0))
    then:
        its_picture = (window.pictures_drawn() == 1)


-- A card that is matched still shows its picture.
test a_matched_card_shows_its_picture
        covers draw_card > shown.matched:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 1s)
    given:
        made = picture.from_svg(stand_in.drawing, size: layout.picture_size) or fail
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.pictures := [made]
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 2, clock.now())
    when:
        draw_card(game, 2, corner_of(0, 1))
    then:
        its_picture = (window.pictures_drawn() == 1)


-- Filling a card's square fills one square, and draws no picture and writes nothing.
test filling_a_card_fills_one_square
        covers fill_card:
    inject:
            window = fake window(frames: [])
    when:
        fill_card(corner_of(0, 0), palette.back)
    then:
        one_square = (window.fills() == 1)
        no_picture = (window.pictures_drawn() == 0)
        no_text = window.written().is_empty()


-- A card's picture is drawn on it.
test a_card_picture_is_drawn
        covers draw_picture:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 1s)
    given:
        made = picture.from_svg(stand_in.drawing, size: layout.picture_size) or fail
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.pictures := [made]
    when:
        draw_picture(game, 1, corner_of(0, 0))
    then:
        one_picture = (window.pictures_drawn() == 1)


-- While the pictures are fetched, the window says so, and shows a frame.
test fetching_is_shown_in_a_frame
        covers show_fetching:
    inject:
            window = fake window(frames: [frame()])
            forever = fake forever
    when:
        show_fetching()
    then:
        told = (window.written() == ["Fetching pictures from OpenMoji..."])
        one_frame = (window.frames_shown() == 1)


-- A new game fetches a picture for each pair, and deals two cards of each, face down.
test a_new_game_deals_two_cards_of_each_picture
        covers new_game > code'many > number'many:
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(text: stand_in.drawing)
    when:
        started = new_game() or fail
    then:
        a_picture_for_each_pair = (started.pictures.count().to_uint32() == board.pairs)
        two_of_each = (started.pairs.sorted() == stand_in.side_by_side)
        face_down = (started.sides == list.repeated(side.face_down, times: board.cards))
        playing = (standing(started) == outcome.playing)


-- A site that answers with a failure fails the setup.
test a_site_that_fails_fails_the_setup
        covers new_game > code > drawing'fails:
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(status: 404)
    when:
        refused = new_game()
    then:
        unreachable = (refused == result.err(setup_error.unreachable(error: http_error.fake_status(404))))


-- A site slower than the fetch time fails the setup.
test a_site_too_slow_fails_the_setup:
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(expiring: true)
    when:
        refused = new_game()
    then:
        timed_out = (refused == result.err(setup_error.timed_out(deadline: expired.fake())))


-- A site that gives what is not a drawing fails the setup.
test what_is_not_a_drawing_fails_the_setup
        covers new_game > code > made'fails:
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(text: "not a picture")
    given:
        expected = picture.from_svg("not a picture", size: layout.picture_size)
    when:
        refused = new_game()
    then:
        not_a_picture = (refused == result.err(setup_error.not_a_picture(error: expected.failure())))


-- A site that fails after giving some of the pictures fails the setup.
test a_site_that_fails_partway_fails_the_setup
        covers new_game > code'many > drawing'fails:
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(text: stand_in.drawing, answers: 3, then_status: 404)
    when:
        refused = new_game()
    then:
        unreachable = (refused == result.err(setup_error.unreachable(error: http_error.fake_status(404))))


-- A site that gives what is not a drawing, after a drawing, fails the setup.
test a_site_that_gives_a_bad_picture_partway_fails_the_setup
        covers new_game > code'many > made'fails:
    given:
        expected = picture.from_svg("not a picture", size: layout.picture_size)
    inject:
            random = fake random(seed: 7)
            clock = fake clock(tick: 100ms)
            site = fake http(texts: [stand_in.drawing, "not a picture"])
    when:
        refused = new_game()
    then:
        not_a_picture = (refused == result.err(setup_error.not_a_picture(error: expected.failure())))


-- A site that answers with a failure ends play before the first game.
test a_site_that_fails_ends_play
        covers play > game'1'fails:
    inject:
            window = fake window(frames: [frame()])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(status: 404)
            forever = fake forever
    when:
        ended = play()
    then:
        unreachable = (ended == result.err(setup_error.unreachable(error: http_error.fake_status(404))))


-- A site slower than the fetch time ends play before the first game.
test a_site_too_slow_ends_play:
    inject:
            window = fake window(frames: [frame()])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(expiring: true)
            forever = fake forever
    when:
        ended = play()
    then:
        timed_out = (ended == result.err(setup_error.timed_out(deadline: expired.fake())))


-- A site that gives what is not a drawing ends play before the first game.
test what_is_not_a_drawing_ends_play:
    given:
        expected = picture.from_svg("not a picture", size: layout.picture_size)
    inject:
            window = fake window(frames: [frame()])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: "not a picture")
            forever = fake forever
    when:
        ended = play()
    then:
        not_a_picture = (ended == result.err(setup_error.not_a_picture(error: expected.failure())))


-- A window closed while the pictures are fetched ends play before the first frame of a game.
test a_window_closed_while_fetching_ends_play
        covers play > closed_at_once'2:
    inject:
            window = fake window(frames: [])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        no_score = (best == 0)
        no_game_drawn = (window.written() == ["Fetching pictures from OpenMoji..."])


-- A window closed after one frame of a game ends play.
test a_window_closed_after_one_frame_ends_play
        covers play > open > moment.picking:
    inject:
            window = fake window(frames: [frame(), frame()])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        no_score = (best == 0)
        one_frame_of_the_game = window.written().contains("Pairs 0 of 10   Lives 10   Time 0s   Best 0")
        two_frames = (window.frames_shown() == 2)


-- A game goes on, frame after frame, while the player clicks nothing.
test a_game_waits_for_clicks
        covers play > open'many > moment.picking:
    inject:
            window = fake window(frames: [frame(), frame(), frame(), frame()])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        no_score = (best == 0)
        no_card_up = (window.pictures_drawn() == 0)
        four_frames = (window.frames_shown() == 4)


-- Two cards that do not match show for a second, while a click on a third is ignored, then turn back.
test a_pair_that_does_not_match_is_shown_then_turned_back
        covers play > open'many > moment.hiding'2eq:
    given:
        mutable frames = [frame(), clicking(row: 0, column: 0), clicking(row: 0, column: 1), clicking(row: 0, column: 2)]
        frames.extend(list.repeated(frame(), times: 9))
        frames.push(frame(keys: [key.escape]))
    inject:
            window = fake window(frames: frames)
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        no_score = (best == 0)
        a_life_lost = window.written().contains("Pairs 0 of 10   Lives 9   Time 0s   Best 0")
        credited = (window.written().last() == optional.some("Pictures: OpenMoji, CC BY-SA 4.0"))

        -- Expected one picture, then two in each frame for a second, then none.
        turned_back = (window.pictures_drawn() == 21)


-- Two cards that do not match turn back in the first frame after their second is up, even when
-- the frames are slow.
test a_pair_shown_past_its_second_turns_back
        covers play > open'many > moment.hiding'2gt:
    inject:
            window = fake window(frames: [
                frame(),
                clicking(row: 0, column: 0), clicking(row: 0, column: 1),
                frame(), frame(), frame(), frame(),
                frame(keys: [key.escape]),
            ])
            clock = fake clock(tick: 300ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        no_score = (best == 0)
        a_life_lost = window.written().contains("Pairs 0 of 10   Lives 9   Time 1s   Best 0")


-- A player who turns over the pairs in order wins, and the score stands until the window is closed.
test a_won_game_keeps_its_score
        covers play > open'many > moment.over:
    given:
        mutable frames = every_pair_clicked()
        frames.push(frame())
        frames.push(frame(keys: [key.escape]))
    inject:
            window = fake window(frames: frames)
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        -- Expected 1000 for the pairs, 500 for the lives and 1180 for the seconds under par.
        scored = (best == 2680)
        announced = window.written().contains("All pairs in 2s! Score 2680   Best 2680")


-- A player who has won presses Space, and a new game is dealt, with new pictures.
test a_whole_game_is_won_and_another_begun
        covers play > open'many > moment.again:
    given:
        mutable frames = every_pair_clicked()
        frames.push(frame())
        frames.push(frame(keys: [key.space]))
        frames.push(frame())
        frames.push(frame(keys: [key.escape]))
    inject:
            window = fake window(frames: frames)
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing)
            forever = fake forever
    when:
        best = play() or fail
    then:
        scored = (best == 2680)
        dealt_again = window.written().contains("Pairs 0 of 10   Lives 10   Time 0s   Best 2680")


-- A site that gives the pictures for one game, and fails for the next, ends play when Space asks for it.
test a_site_that_fails_for_the_next_game_ends_play
        covers play > open'many > moment.again > game'2'fails:
    given:
        mutable frames = every_pair_clicked()
        frames.push(frame())
        frames.push(frame(keys: [key.space]))
    inject:
            window = fake window(frames: frames)
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing, answers: 10, then_status: 404)
            forever = fake forever
    when:
        ended = play()
    then:
        unreachable = (ended == result.err(setup_error.unreachable(error: http_error.fake_status(404))))
