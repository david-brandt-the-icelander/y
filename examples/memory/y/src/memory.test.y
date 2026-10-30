-- What stands in for the pictures in the tests.
constants stand_in:
    -- A drawing that a fake site gives for every picture: a red circle.
    drawing = "<svg viewBox='0 0 72 72' xmlns='http://www.w3.org/2000/svg'><circle cx='36' cy='36' r='30' fill='red'/></svg>"

    -- The order of the cards for the rules: each picture's two cards side by side.
    side_by_side is list<uint32> = [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10]


--------------------------------------------------------------------------------------------------------------------------------
A game dealt with its cards in a given order, all face down, with no pictures: for the rules, which draw nothing.

pairs: For each card, the number of the picture it shows, as list<uint32>.
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


-- A click between the cards chooses no card.
test a_click_between_the_cards_chooses_nothing
        covers choosable > judged.off_cards, corner_of:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        choice = choosable(game, point(x: 6, y: 6))
    then:
        nothing = (choice == optional.none)


-- A click on a card that is face down chooses it.
test a_click_on_a_card_chooses_it
        covers choosable > judged.free, card_at > row > column'many > inside:
    inject:
            clock = fake clock(tick: 1s)
    given:
        game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        choice = choosable(game, point(x: 192, y: 68))
    then:
        the_second_card = (choice == optional.some(2))


-- A card turned face up cannot be chosen again.
test a_face_up_card_cannot_be_chosen_again
        covers choosable > judged.showing:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        turn_over(mutable game, 2, clock.now())
        choice = choosable(game, point(x: 192, y: 68))
    then:
        face_up = (game.sides.at_number(2) == side.face_up)
        not_again = (choice == optional.none)


-- Two cards with the same picture stay face up, and cost no life.
test a_matching_pair_stays_face_up
        covers turn_over > turn.first, turn_over > turn.matching:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 2, clock.now())
    then:
        first_matched = (game.sides.at_number(1) == side.matched)
        second_matched = (game.sides.at_number(2) == side.matched)
        one_pair = (game.matches == 1)
        a_new_turn = game.turned.is_empty()
        no_life_lost = (game.lives == game_rules.lives)


-- Two cards with different pictures cost a life, and no third card can be chosen while they show.
test a_pair_that_does_not_match_costs_a_life
        covers choosable > judged.waiting:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 3, clock.now())
        choice = choosable(game, point(x: 564, y: 68))
    then:
        a_life_lost = (game.lives == game_rules.lives - 1)
        both_showing = (game.turned == [1, 3])
        no_third_card = (choice == optional.none)


-- Two cards that do not match turn face down again.
test two_cards_that_do_not_match_turn_back
        covers turn_over > turn.different, turn_back > number'many:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
    when:
        turn_over(mutable game, 1, clock.now())
        turn_over(mutable game, 3, clock.now())
        turn_back(mutable game)
    then:
        first_face_down = (game.sides.at_number(1) == side.face_down)
        second_face_down = (game.sides.at_number(3) == side.face_down)
        a_new_turn = game.turned.is_empty()


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


-- A game won under the par time scores its pairs, its lives left and the seconds under par.
test a_quick_win_scores_lives_and_seconds
        covers score > won, time_bonus > quick:
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


-- A game won over the par time scores its pairs and its lives left, and nothing for the time.
test a_slow_win_scores_no_seconds
        covers time_bonus > slow'gt:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        game.matches := board.pairs
        game.lives := 4
        game.elapsed := 150s
    when:
        points = score(game)
    then:
        pairs_and_lives = (points == 1200)


-- The banner says how the game stands, and the score once it has ended.
test the_banner_says_how_the_game_stands
        covers banner > shown.playing, banner > shown.won, banner > shown.lost:
    inject:
            clock = fake clock(tick: 1s)
    given:
        playing = dealt(stand_in.side_by_side, at: clock.now())
        mutable won = dealt(stand_in.side_by_side, at: clock.now())
        won.matches := board.pairs
        won.lives := 4
        won.elapsed := 100s
        mutable lost = dealt(stand_in.side_by_side, at: clock.now())
        lost.matches := 3
        lost.lives := 0
    when:
        playing_lines = banner(playing, 50)
        won_lines = banner(won, 1400)
        lost_lines = banner(lost, 50)
    then:
        says_playing = (playing_lines[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50")
        says_won = (won_lines[0] == "All pairs in 100s! Score 1400   Best 1400")
        says_lost = (lost_lines[0] == "Out of lives with 3 pairs. Score 300   Best 50")


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


-- A player who turns over the pairs in order wins, and Space deals a new game. With the seed 7, the first card of each
-- pair is clicked, then the second.
test a_whole_game_is_won_and_another_begun
        covers play > open'many > moment.again, play > open'many > moment.over:
    inject:
            window = fake window(frames: [
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
                frame(),
                frame(keys: [key.space]),
                frame(),
                frame(keys: [key.escape]),
            ])
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
        pictures_shown = (window.pictures_drawn() > 0)


-- Two cards that do not match show for a second, while a click on a third is ignored, then turn back.
test a_pair_that_does_not_match_is_shown_then_turned_back
        covers play > open'many > moment.hiding'2eq, pick > place > ignored:
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


-- A site that answers with a failure ends play before the first game.
test a_site_that_fails_ends_play
        covers show_fetching, play > game'1'fails:
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


-- A click below a card, in the gap above the next row, chooses no card.
test a_click_between_two_rows_chooses_nothing
        covers card_at > row'many > column'many > outside'1, card_at > row'many > column'many > outside'2:
    when:
        found = card_at(point(x: 68, y: 130))
    then:
        nothing = (found == optional.none)


-- A game won in exactly the par time scores nothing for the time.
test a_game_exactly_on_par_scores_no_seconds
        covers time_bonus > slow'eq:
    when:
        bonus = time_bonus(game_rules.par_time)
    then:
        nothing = (bonus == 0)


-- Two clicks in one frame turn over two cards; a third, on a card already up, is ignored.
test two_clicks_in_a_frame_turn_two_cards
        covers card_at > row > column > inside, pick > place'many > chosen, pick > place'many > ignored:
    inject:
            clock = fake clock(tick: 1s)
    given:
        mutable game = dealt(stand_in.side_by_side, at: clock.now())
        mutable again = dealt(stand_in.side_by_side, at: clock.now())
    when:
        pick(mutable game, [point(x: 68, y: 68), point(x: 192, y: 68)], clock.now())
        pick(mutable again, [point(x: 68, y: 68), point(x: 68, y: 68)], clock.now())
    then:
        a_pair = (game.matches == 1)
        one_up = (again.turned == [1])


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


-- A window closed after one frame of a game ends play.
test a_window_closed_after_one_frame_ends_play
        covers draw > row'many > column'many, draw_card > shown.face_down, fill_card, play > open > moment.picking,
               pick > no_clicks:
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


-- A site that gives the pictures for one game, and fails for the next, ends play when Space asks for it. With the
-- seed 7, the first card of each pair is clicked, then the second.
test a_site_that_fails_for_the_next_game_ends_play
        covers card_at > row'many > column > inside, card_at > row'many > column'many > inside,
               draw_card > shown.matched, play > open'many > moment.again > game'2'fails:
    inject:
            window = fake window(frames: [
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
                frame(),
                frame(keys: [key.space]),
            ])
            clock = fake clock(tick: 100ms)
            random = fake random(seed: 7)
            site = fake http(text: stand_in.drawing, answers: 10, then_status: 404)
            forever = fake forever
    when:
        ended = play()
    then:
        unreachable = (ended == result.err(setup_error.unreachable(error: http_error.fake_status(404))))


-- Two cards that do not match turn back in the first frame after their second is up, even when
-- the frames are slow.
test a_pair_shown_past_its_second_turns_back
        covers draw_card > shown.face_up, draw_picture, play > open'many > moment.hiding'2gt,
               play > open'many > moment.picking, pick > place > chosen:
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
