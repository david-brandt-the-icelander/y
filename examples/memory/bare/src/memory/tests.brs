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
        pairs: pairs.to_vec(),
        sides: list::repeated(Side::FaceDown, board::CARDS),
        pictures: Vec::new(),
        turned: Vec::new(),
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

#[test]
fn a_click_between_the_cards_chooses_nothing() {
    // y: covers choosable#judged.off_cards, card_at#outside
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    let choice = choosable(&game, Point { x: 6, y: 6 });
    assert!(choice.is_none(), "nothing");
}

#[test]
fn a_click_on_a_card_chooses_it() {
    // y: covers choosable#judged.free, card_at#inside
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    let choice = choosable(&game, Point { x: 192, y: 68 });
    assert!(choice == Some(2), "the_second_card");
}

#[test]
fn a_face_up_card_cannot_be_chosen_again() {
    // y: covers choosable#judged.showing, turn_over#turn.first
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    turn_over(&mut game, 2, clock.now());
    let choice = choosable(&game, Point { x: 192, y: 68 });
    assert!(list::at_number(&game.sides, 2) == Side::FaceUp, "face_up");
    assert!(choice.is_none(), "not_again");
}

#[test]
fn a_matching_pair_stays_face_up() {
    // y: covers turn_over#turn.matching
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
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
    // y: covers turn_over#turn.different, choosable#judged.waiting
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 3, clock.now());
    let choice = choosable(&game, Point { x: 564, y: 68 });
    assert!(game.lives == game_rules::LIVES - 1, "a_life_lost");
    assert!(game.turned == vec![1, 3], "both_showing");
    assert!(choice.is_none(), "no_third_card");
}

#[test]
fn two_cards_that_do_not_match_turn_back() {
    // y: covers turn_over#turn.different
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    turn_over(&mut game, 1, clock.now());
    turn_over(&mut game, 3, clock.now());
    turn_back(&mut game);
    assert!(list::at_number(&game.sides, 1) == Side::FaceDown, "first_face_down");
    assert!(list::at_number(&game.sides, 3) == Side::FaceDown, "second_face_down");
    assert!(game.turned.is_empty(), "a_new_turn");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers standing#judged.playing
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    let reached = standing(&game);
    assert!(reached == Outcome::Playing, "playing");
}

#[test]
fn finding_every_pair_wins() {
    // y: covers standing#judged.won
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    game.matches = board::PAIRS;
    let reached = standing(&game);
    assert!(reached == Outcome::Won, "won");
}

#[test]
fn losing_every_life_loses() {
    // y: covers standing#judged.lost
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    game.lives = 0;
    let reached = standing(&game);
    assert!(reached == Outcome::Lost, "lost");
}

#[test]
fn an_unfinished_game_scores_its_pairs() {
    // y: covers score#unfinished
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    game.matches = 3;
    game.lives = 0;
    let points = score(&game);
    assert!(points == 300, "three_pairs");
}

#[test]
fn a_quick_win_scores_lives_and_seconds() {
    // y: covers score#won, time_bonus#quick
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    game.matches = board::PAIRS;
    game.lives = 4;
    game.elapsed = Duration::from_secs(100);
    let points = score(&game);
    assert!(points == 1400, "pairs_lives_and_seconds: Expected 1000 for the pairs, 200 for the lives and 200 for the seconds.");
}

#[test]
fn a_slow_win_scores_no_seconds() {
    // y: covers time_bonus#slow
    let clock = Clock::fake(Duration::from_secs(1));
    let game = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    game.matches = board::PAIRS;
    game.lives = 4;
    game.elapsed = Duration::from_secs(150);
    let points = score(&game);
    assert!(points == 1200, "pairs_and_lives");
}

#[test]
fn the_banner_says_how_the_game_stands() {
    // y: covers banner#shown.playing, banner#shown.won, banner#shown.lost
    let clock = Clock::fake(Duration::from_secs(1));
    let playing = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    let won = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    won.matches = board::PAIRS;
    won.lives = 4;
    won.elapsed = Duration::from_secs(100);
    let lost = dealt(&stand_in::SIDE_BY_SIDE, clock.now());
    lost.matches = 3;
    lost.lives = 0;
    let playing_lines = banner(&playing, 50);
    let won_lines = banner(&won, 1400);
    let lost_lines = banner(&lost, 50);
    assert!(playing_lines[0] == "Pairs 0 of 10   Lives 10   Time 0s   Best 50", "says_playing");
    assert!(won_lines[0] == "All pairs in 100s! Score 1400   Best 1400", "says_won");
    assert!(lost_lines[0] == "Out of lives with 3 pairs. Score 300   Best 50", "says_lost");
}

#[test]
fn a_new_game_deals_two_cards_of_each_picture() {
    // y: covers standing#judged.playing
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake(stand_in::DRAWING);
    let started = y_window::block_on(new_game(&mut random, &mut clock, &mut site)).expect("when");
    let sorted = started.pairs.clone();
    sorted.sort();
    assert!(list::count(&started.pictures) == board::PAIRS, "a_picture_for_each_pair");
    assert!(sorted == stand_in::SIDE_BY_SIDE, "two_of_each");
    assert!(started.sides == list::repeated(Side::FaceDown, board::CARDS), "face_down");
    assert!(standing(&started) == Outcome::Playing, "playing");
}

#[test]
fn a_site_that_fails_fails_the_setup() {
    // y: covers new_game!setup_error.unreachable
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_failing(404);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &mut site));
    assert!(refused == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}

#[test]
fn a_site_too_slow_fails_the_setup() {
    // y: covers new_game!setup_error.timed_out
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake_expiring();
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &mut site));
    assert!(refused == Err(SetupError::TimedOut { deadline: Expired::fake() }), "timed_out");
}

#[test]
fn what_is_not_a_drawing_fails_the_setup() {
    // y: covers new_game!setup_error.not_a_picture
    let random = Random::seeded(7);
    let clock = Clock::fake(Duration::from_millis(100));
    let site = Http::fake("not a picture");
    let expected = Picture::from_svg("not a picture", layout::PICTURE_SIZE);
    let refused = y_window::block_on(new_game(&mut random, &mut clock, &mut site));
    assert!(refused == Err(SetupError::NotAPicture { error: expected.expect_err("given") }), "not_a_picture");
}

#[test]
fn a_whole_game_is_won_and_another_begun() {
    // y: covers play#open, play#moment.picking, play#chosen, play#moment.over, play#moment.again, play#closed,
    // draw_card#shown.face_down, draw_card#shown.face_up, draw_card#shown.matched, banner#shown.won
    let window = Window::fake_frames(vec![
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
        Frame::default(),
        Frame { keys: vec![Key::Space], clicks: Vec::new() },
        Frame::default(),
        Frame { keys: vec![Key::Escape], clicks: Vec::new() },
    ]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &mut site, &forever)).expect("when");
    let written = window.written();
    assert!(best == 2680, "scored: Expected 1000 for the pairs, 500 for the lives and 1180 for the seconds under par.");
    assert!(written.iter().any(|line| line == "All pairs in 2s! Score 2680   Best 2680"), "announced");
    assert!(window.pictures_drawn() > 0, "pictures_shown");
}

#[test]
fn a_pair_that_does_not_match_is_shown_then_turned_back() {
    // y: covers play#ignored, play#moment.hiding
    let frames = vec![Frame::default(), clicking(0, 0), clicking(0, 1), clicking(0, 2)];
    frames.extend(std::iter::repeat_n(Frame::default(), 9));
    frames.push(Frame { keys: vec![Key::Escape], clicks: Vec::new() });
    let window = Window::fake_frames(frames);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake(stand_in::DRAWING);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut clock, &mut random, &mut site, &forever)).expect("when");
    let written = window.written();
    assert!(best == 0, "no_score");
    assert!(written.iter().any(|line| line == "Pairs 0 of 10   Lives 9   Time 0s   Best 0"), "a_life_lost");
    assert!(written.last().map(String::as_str) == Some("Pictures: OpenMoji, CC BY-SA 4.0"), "credited");
    assert!(window.pictures_drawn() == 21, "turned_back: Expected one picture, then two in each frame for a second, then none.");
}

#[test]
fn a_site_that_fails_ends_play() {
    // y: covers play!setup_error.unreachable
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake_failing(404);
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &mut site, &forever));
    assert!(ended == Err(SetupError::Unreachable { error: HttpError::fake_status(404) }), "unreachable");
}

#[test]
fn a_site_too_slow_ends_play() {
    // y: covers play!setup_error.timed_out
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake_expiring();
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &mut site, &forever));
    assert!(ended == Err(SetupError::TimedOut { deadline: Expired::fake() }), "timed_out");
}

#[test]
fn what_is_not_a_drawing_ends_play() {
    // y: covers play!setup_error.not_a_picture
    let expected = Picture::from_svg("not a picture", layout::PICTURE_SIZE);
    let window = Window::fake_frames(vec![Frame::default()]);
    let clock = Clock::fake(Duration::from_millis(100));
    let random = Random::seeded(7);
    let site = Http::fake("not a picture");
    let forever = Forever::fake();
    let ended = y_window::block_on(play(&mut window, &mut clock, &mut random, &mut site, &forever));
    assert!(ended == Err(SetupError::NotAPicture { error: expected.expect_err("given") }), "not_a_picture");
}
