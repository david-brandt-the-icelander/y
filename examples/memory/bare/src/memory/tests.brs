//! Lowered from `y/src/memory.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test. A test of a function
//! that may suspend runs it with `y_window::block_on`, as its capabilities are fakes, which never suspend.

use super::window_play::*;
use super::*;
use y_window::Frame;

/// What stands in for the pictures in the tests: the Y constant group `stand_in`.
mod stand_in {
    /// A drawing that a fake site gives for every picture: a red circle.
    const DRAWING = "<svg viewBox='0 0 72 72' xmlns='http://www.w3.org/2000/svg'><circle cx='36' cy='36' r='30' fill='red'/></svg>";
    /// The order of the cards for the rules: each picture's two cards side by side, `[1, 1, 2, 2, …]`.
    const SIDE_BY_SIDE = [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10];
}

/// A game dealt with its cards in a given order, all face down, with no pictures: for the rules, which draw nothing.
///
/// - `pairs`: for each card, the number of the picture it shows.
/// - `at`: when the game started.
/// - Returns the game.
fn dealt(pairs, at) {
    Game {
        pairs: pairs.clone(),
        sides: List::from(list::repeated(Side::FaceDown, board::CARDS)),
        pictures: List::new(),
        turned: List::new(),
        turned_at: at,
        matches: 0,
        lives: game_rules::LIVES,
        started: at,
        elapsed: Duration::ZERO,
    }
}

/// A frame of a fake window in which the player clicks the middle of a card.
///
/// - `row`: the card's row, counted from 0.
/// - `column`: the card's column, counted from 0.
/// - Returns the frame.
fn clicking(row, column) {
    let corner = corner_of(row, column);
    let half = layout::CARD_SIZE / 2;
    Frame { keys: Vec::new(), clicks: vec![Point { x: corner.x + half, y: corner.y + half }] }
}

/// The frames of a fake window in which the player turns over every pair, with the seed 7: the first card of each pair
/// is clicked, then the second.
///
/// - Returns the frames.
fn every_pair_clicked() {
    list![
        Frame::default(),
        clicking(3, 1), clicking(3, 2),
        clicking(0, 0), clicking(2, 3),
        clicking(1, 1), clicking(3, 3),
        clicking(0, 3), clicking(3, 4),
        clicking(1, 0), clicking(2, 1),
        clicking(2, 0), clicking(2, 2),
        clicking(1, 3), clicking(2, 4),
        clicking(0, 4), clicking(3, 0),
        clicking(1, 2), clicking(1, 4),
        clicking(0, 1), clicking(0, 2),
    ]
}

#[test]
fn a_click_between_the_cards_chooses_nothing() {
    // y: covers choosable > judged.off_cards
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    let choice = choosable(&game, Point { x: 6, y: 6 });
    assert!(choice.is_none(), "nothing");
}

#[test]
fn no_third_card_can_be_chosen_while_two_show() {
    // y: covers choosable > judged.waiting
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 3, clock.now());
    let choice = choosable(&game, Point { x: 564, y: 68 });
    assert!(choice.is_none(), "no_third_card");
}

#[test]
fn a_face_up_card_cannot_be_chosen_again() {
    // y: covers choosable > judged.showing
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 2, clock.now());
    let choice = choosable(&game, Point { x: 192, y: 68 });
    assert!(choice.is_none(), "not_again");
}

#[test]
fn a_click_on_a_card_chooses_it() {
    // y: covers choosable > judged.free
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    let choice = choosable(&game, Point { x: 192, y: 68 });
    assert!(choice == Some(2), "the_second_card");
}

#[test]
fn the_first_card_of_a_turn_turns_face_up() {
    // y: covers turn_over > turn.first
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 1, clock.now());
    assert!(list::at_number(&game.sides, 1) == Side::FaceUp, "face_up");
    assert!(game.turned == list![1], "waiting_for_the_second");
    assert!(game.lives == game_rules::LIVES, "no_life_lost");
}

#[test]
fn a_matching_pair_stays_face_up() {
    // y: covers turn_over > turn.matching
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 2, clock.now());
    assert!(list::at_number(&game.sides, 1) == Side::Matched, "first_matched");
    assert!(list::at_number(&game.sides, 2) == Side::Matched, "second_matched");
    assert!(game.matches == 1, "one_pair");
    assert!(game.turned.is_empty(), "a_new_turn");
    assert!(game.lives == game_rules::LIVES, "no_life_lost");
}

#[test]
fn a_pair_that_does_not_match_costs_a_life() {
    // y: covers turn_over > turn.different
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 3, clock.now());
    assert!(list::at_number(&game.sides, 3) == Side::FaceUp, "second_face_up");
    assert!(game.turned == list![1, 3], "both_showing");
    assert!(game.matches == 0, "no_pair");
    assert!(game.lives == game_rules::LIVES - 1, "a_life_lost");
}

#[test]
fn two_cards_that_do_not_match_turn_back() {
    // y: covers turn_back > number'many
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 3, clock.now());
    turn_back(&mut game);
    assert!(list::at_number(&game.sides, 1) == Side::FaceDown, "first_face_down");
    assert!(list::at_number(&game.sides, 3) == Side::FaceDown, "second_face_down");
    assert!(game.turned.is_empty(), "a_new_turn");
}

#[test]
fn finding_every_pair_wins() {
    // y: covers standing > judged.won
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.matches = board::PAIRS;
    let reached = standing(&game);
    assert!(reached == Outcome::Won, "won");
}

#[test]
fn losing_every_life_loses() {
    // y: covers standing > judged.lost
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.lives = 0;
    let reached = standing(&game);
    assert!(reached == Outcome::Lost, "lost");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers standing > judged.playing
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    let reached = standing(&game);
    assert!(reached == Outcome::Playing, "playing");
}

#[test]
fn a_won_game_scores_lives_and_seconds() {
    // y: covers score > won
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.matches = board::PAIRS;
    game.lives = 4;
    game.elapsed = Duration::from_secs(100);
    let points = score(&game);
    assert!(points == 1400, "pairs_lives_and_seconds: Expected 1000 for the pairs, 200 for the lives and 200 for the seconds.");
}

#[test]
fn an_unfinished_game_scores_its_pairs() {
    // y: covers score > unfinished
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.matches = 3;
    game.lives = 0;
    let points = score(&game);
    assert!(points == 300, "three_pairs");
}

#[test]
fn a_game_under_par_scores_its_seconds() {
    // y: covers time_bonus > quick
    let bonus = time_bonus(Duration::from_secs(100));
    assert!(bonus == 200, "twenty_seconds");
}

#[test]
fn a_game_exactly_on_par_scores_no_seconds() {
    // y: covers time_bonus > slow'eq
    let bonus = time_bonus(game_rules::PAR_TIME);
    assert!(bonus == 0, "nothing");
}

#[test]
fn a_game_over_par_scores_no_seconds() {
    // y: covers time_bonus > slow'gt
    let bonus = time_bonus(Duration::from_secs(150));
    assert!(bonus == 0, "nothing");
}

#[test]
fn the_first_card_is_found() {
    // y: covers card_at > row > column > inside
    let found = card_at(Point { x: 68, y: 68 });
    assert!(found == Some(1), "card_one");
}

#[test]
fn a_card_along_the_top_row_is_found() {
    // y: covers card_at > row > column'many > inside
    let found = card_at(Point { x: 192, y: 68 });
    assert!(found == Some(2), "card_two");
}

#[test]
fn a_card_down_the_first_column_is_found() {
    // y: covers card_at > row'many > column > inside
    let found = card_at(Point { x: 68, y: 192 });
    assert!(found == Some(6), "card_six");
}

#[test]
fn a_card_down_and_along_is_found() {
    // y: covers card_at > row'many > column'many > inside
    let found = card_at(Point { x: 192, y: 192 });
    assert!(found == Some(7), "card_seven");
}

#[test]
fn a_click_between_two_columns_finds_nothing() {
    // y: covers card_at > row'many > column'many > outside'1
    let found = card_at(Point { x: 130, y: 68 });
    assert!(found.is_none(), "nothing");
}

#[test]
fn a_click_between_two_rows_finds_nothing() {
    // y: covers card_at > row'many > column'many > outside'2
    let found = card_at(Point { x: 68, y: 130 });
    assert!(found.is_none(), "nothing");
}

#[test]
fn a_card_corner_is_a_step_per_row_and_column() {
    // y: covers corner_of
    let corner = corner_of(1, 2);
    assert!(corner.x == 260, "across");
    assert!(corner.y == 136, "down");
}

#[test]
fn no_clicks_turn_nothing() {
    // y: covers pick > no_clicks
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    pick(&mut game, &List::new(), clock.now());
    assert!(game.turned.is_empty(), "nothing_turned");
}

#[test]
fn a_click_on_a_card_turns_it() {
    // y: covers pick > place > chosen
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    pick(&mut game, &list![Point { x: 68, y: 68 }], clock.now());
    assert!(game.turned == list![1], "first_up");
}

#[test]
fn a_click_between_the_cards_is_ignored() {
    // y: covers pick > place > ignored
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    pick(&mut game, &list![Point { x: 6, y: 6 }], clock.now());
    assert!(game.turned.is_empty(), "nothing_turned");
}

#[test]
fn two_clicks_in_a_frame_turn_two_cards() {
    // y: covers pick > place'many > chosen
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    pick(&mut game, &list![Point { x: 68, y: 68 }, Point { x: 192, y: 68 }], clock.now());
    assert!(game.matches == 1, "a_pair");
}

#[test]
fn two_clicks_between_the_cards_are_ignored() {
    // y: covers pick > place'many > ignored
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    pick(&mut game, &list![Point { x: 6, y: 6 }, Point { x: 130, y: 68 }], clock.now());
    assert!(game.turned.is_empty(), "nothing_turned");
}

#[test]
fn the_banner_says_how_a_game_is_going() {
    // y: covers banner > shown.playing
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    let lines = banner(&game, 50);
    assert!(lines[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50", "says_playing");
    assert!(lines[1] == "Click two cards to turn them over. Escape quits.", "says_how");
}

#[test]
fn the_banner_says_a_game_is_won() {
    // y: covers banner > shown.won
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.matches = board::PAIRS;
    game.lives = 4;
    game.elapsed = Duration::from_secs(100);
    let lines = banner(&game, 1400);
    assert!(lines[0] == "All pairs in 100s! Score 1400   Best 1400", "says_won");
    assert!(lines[1] == "Space plays again. Escape quits.", "says_again");
}

#[test]
fn the_banner_says_a_game_is_lost() {
    // y: covers banner > shown.lost
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.matches = 3;
    game.lives = 0;
    let lines = banner(&game, 50);
    assert!(lines[0] == "Out of lives with 3 pairs. Score 300   Best 50", "says_lost");
    assert!(lines[1] == "Space plays again. Escape quits.", "says_again");
}

#[test]
fn a_game_is_drawn_with_its_banner_and_credit() {
    // y: covers draw > row'many > column'many
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    draw(&game, 50, &mut window);
    let written = window.written().iter().map(|line| YString::new(line)).collect();
    assert!(written[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50", "banner_written");
    assert!(written.last().map(YString::as_str) == Some("Pictures: OpenMoji, CC BY-SA 4.0"), "credited");
    assert!(window.pictures_drawn() == 0, "all_face_down");
}

#[test]
fn a_face_down_card_shows_no_picture() {
    // y: covers draw_card > shown.face_down
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    draw_card(&game, 1, corner_of(0, 0), &mut window);
    assert!(window.pictures_drawn() == 0, "no_picture");
}

#[test]
fn a_face_up_card_shows_its_picture() {
    // y: covers draw_card > shown.face_up
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_secs(1));
    let made = Picture::from_svg(stand_in::DRAWING, layout::PICTURE_SIZE).expect("given");
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.pictures = list![made];
    turn_over(&mut game, 1, clock.now());
    draw_card(&game, 1, corner_of(0, 0), &mut window);
    assert!(window.pictures_drawn() == 1, "its_picture");
}

#[test]
fn a_matched_card_shows_its_picture() {
    // y: covers draw_card > shown.matched
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_secs(1));
    let made = Picture::from_svg(stand_in::DRAWING, layout::PICTURE_SIZE).expect("given");
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.pictures = list![made];
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 2, clock.now());
    draw_card(&game, 2, corner_of(0, 1), &mut window);
    assert!(window.pictures_drawn() == 1, "its_picture");
}

#[test]
fn filling_a_card_fills_one_square() {
    // y: covers fill_card
    let window = Window::fake_frames(Vec::new());
    fill_card(corner_of(0, 0), palette::BACK, &mut window);
    assert!(window.fills() == 1, "one_square");
    assert!(window.pictures_drawn() == 0, "no_picture");
    assert!(window.written().is_empty(), "no_text");
}

#[test]
fn a_card_picture_is_drawn() {
    // y: covers draw_picture
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_secs(1));
    let made = Picture::from_svg(stand_in::DRAWING, layout::PICTURE_SIZE).expect("given");
    let game = dealt(&List::from(stand_in::SIDE_BY_SIDE), clock.now());
    game.pictures = list![made];
    draw_picture(&game, 1, corner_of(0, 0), &mut window);
    assert!(window.pictures_drawn() == 1, "one_picture");
}

#[test]
fn fetching_is_shown_in_a_frame() {
    // y: covers show_fetching
    let window = Window::fake_frames(vec![Frame::default()]);
    let forever = Forever::fake();
    y_window::block_on(show_fetching(&mut window, &forever));
    assert!(window.written() == ["Fetching pictures from OpenMoji..."], "told");
    assert!(window.frames_shown() == 1, "one_frame");
}

#[test]
fn a_new_game_deals_two_cards_of_each_picture() {
    // y: covers new_game > code'many > number'many
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake(stand_in::DRAWING);
    let started = y_window::block_on(new_game(&mut random, &mut clock, &site)).expect("when");
    let sorted = started.pairs.clone();
    sorted.sort();
    assert!(list::count(&started.pictures) == board::PAIRS, "a_picture_for_each_pair");
    assert!(sorted == List::from(stand_in::SIDE_BY_SIDE), "two_of_each");
    assert!(started.sides == List::from(list::repeated(Side::FaceDown, board::CARDS)), "face_down");
    assert!(standing(&started) == Outcome::Playing, "playing");
}

#[test]
fn a_site_that_fails_fails_the_setup() {
    // y: covers new_game > code > drawing'fails
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_failing(404);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &site));
    assert!(refused == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}

#[test]
fn a_site_too_slow_fails_the_setup() {
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_expiring();
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &site));
    assert!(refused == Err(SetupError::TimedOut { deadline: Expired::fake() }), "timed_out");
}

#[test]
fn what_is_not_a_drawing_fails_the_setup() {
    // y: covers new_game > code > made'fails
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake("not a picture");
    let expected = Picture::from_svg("not a picture", layout::PICTURE_SIZE);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &site));
    assert!(refused == Err(SetupError::NotAPicture { error: expected.expect_err("given") }), "not_a_picture");
}

#[test]
fn a_site_that_fails_partway_fails_the_setup() {
    // y: covers new_game > code'many > drawing'fails
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_failing_after(stand_in::DRAWING, 3, 404);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &site));
    assert!(refused == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}

#[test]
fn a_site_that_gives_a_bad_picture_partway_fails_the_setup() {
    // y: covers new_game > code'many > made'fails
    let expected = Picture::from_svg("not a picture", layout::PICTURE_SIZE);
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_texts(&[stand_in::DRAWING, "not a picture"]);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &site));
    assert!(refused == Err(SetupError::NotAPicture { error: expected.expect_err("given") }), "not_a_picture");
}

#[test]
fn a_site_that_fails_ends_play() {
    // y: covers play > game'1'fails
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake_failing(404);
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever));
    assert!(ended == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}

#[test]
fn a_site_too_slow_ends_play() {
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake_expiring();
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever));
    assert!(ended == Err(SetupError::TimedOut { deadline: Expired::fake() }), "timed_out");
}

#[test]
fn what_is_not_a_drawing_ends_play() {
    let expected = Picture::from_svg("not a picture", layout::PICTURE_SIZE);
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake("not a picture");
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever));
    assert!(ended == Err(SetupError::NotAPicture { error: expected.expect_err("given") }), "not_a_picture");
}

#[test]
fn a_window_closed_while_fetching_ends_play() {
    // y: covers play > closed_at_once'2
    let window = Window::fake_frames(Vec::new());
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    assert!(best == 0, "no_score");
    assert!(window.written() == ["Fetching pictures from OpenMoji..."], "no_game_drawn");
}

#[test]
fn a_window_closed_after_one_frame_ends_play() {
    // y: covers play > open > moment.picking
    let window = Window::fake_frames(vec![Frame::default(), Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    let written = window.written().iter().map(|line| YString::new(line)).collect();
    assert!(best == 0, "no_score");
    assert!(written.iter().any(|line| line == "Pairs 0 of 10   Lives 10   Time 0s   Best 0"), "one_frame_of_the_game");
    assert!(window.frames_shown() == 2, "two_frames");
}

#[test]
fn a_game_waits_for_clicks() {
    // y: covers play > open'many > moment.picking
    let window = Window::fake_frames(vec![Frame::default(), Frame::default(), Frame::default(), Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    assert!(best == 0, "no_score");
    assert!(window.pictures_drawn() == 0, "no_card_up");
    assert!(window.frames_shown() == 4, "four_frames");
}

#[test]
fn a_pair_that_does_not_match_is_shown_then_turned_back() {
    // y: covers play > open'many > moment.hiding'2eq
    let frames = list![Frame::default(), clicking(0, 0), clicking(0, 1), clicking(0, 2)];
    frames.extend(std::iter::repeat_n(Frame::default(), 9));
    frames.push(Frame { keys: vec![Key::Escape], clicks: Vec::new() });
    let window = Window::fake_frames(frames.into_vec());
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    let written = window.written().iter().map(|line| YString::new(line)).collect();
    assert!(best == 0, "no_score");
    assert!(written.iter().any(|line| line == "Pairs 0 of 10   Lives 9   Time 0s   Best 0"), "a_life_lost");
    assert!(written.last().map(YString::as_str) == Some("Pictures: OpenMoji, CC BY-SA 4.0"), "credited");
    assert!(window.pictures_drawn() == 21, "turned_back: Expected one picture, then two in each frame for a second, then none.");
}

#[test]
fn a_pair_shown_past_its_second_turns_back() {
    // y: covers play > open'many > moment.hiding'2gt
    let window = Window::fake_frames(vec![
        Frame::default(),
        clicking(0, 0), clicking(0, 1),
        Frame::default(), Frame::default(), Frame::default(), Frame::default(),
        Frame { keys: vec![Key::Escape], clicks: Vec::new() },
    ]);
    let clock = Clock::fake(Duration::from_millis(300));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "Pairs 0 of 10   Lives 9   Time 1s   Best 0"), "a_life_lost");
}

#[test]
fn a_won_game_keeps_its_score() {
    // y: covers play > open'many > moment.over
    let frames = every_pair_clicked();
    frames.push(Frame::default());
    frames.push(Frame { keys: vec![Key::Escape], clicks: Vec::new() });
    let window = Window::fake_frames(frames.into_vec());
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    let written = window.written().iter().map(|line| YString::new(line)).collect();
    assert!(best == 2680, "scored: Expected 1000 for the pairs, 500 for the lives and 1180 for the seconds under par.");
    assert!(written.iter().any(|line| line == "All pairs in 2s! Score 2680   Best 2680"), "announced");
}

#[test]
fn a_whole_game_is_won_and_another_begun() {
    // y: covers play > open'many > moment.again
    let frames = every_pair_clicked();
    frames.push(Frame::default());
    frames.push(Frame { keys: vec![Key::Space], clicks: Vec::new() });
    frames.push(Frame::default());
    frames.push(Frame { keys: vec![Key::Escape], clicks: Vec::new() });
    let window = Window::fake_frames(frames.into_vec());
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever)).expect("when");
    let written = window.written().iter().map(|line| YString::new(line)).collect();
    assert!(best == 2680, "scored");
    assert!(written.iter().any(|line| line == "Pairs 0 of 10   Lives 10   Time 0s   Best 2680"), "dealt_again");
}

#[test]
fn a_site_that_fails_for_the_next_game_ends_play() {
    // y: covers play > open'many > moment.again > game'2'fails
    let frames = every_pair_clicked();
    frames.push(Frame::default());
    frames.push(Frame { keys: vec![Key::Space], clicks: Vec::new() });
    let window = Window::fake_frames(frames.into_vec());
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake_failing_after(stand_in::DRAWING, 10, 404);
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &site, &forever));
    assert!(ended == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}
