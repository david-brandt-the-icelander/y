//! Lowered from `y/src/checkers.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! A test of a function that may suspend runs it with `y_window::block_on`, as its capabilities are fakes, which never
//! suspend.

use super::window_play::*;
use super::*;
use y_window::Frame;

/// A game with only the given pieces on the board, at the Club level.
///
/// - `pieces`: the pieces and their squares.
/// - `to_move`: whose turn it is.
/// - Returns the game.
fn arranged(pieces: &[Placed], to_move: Side) -> Game {
    let mut squares = list::repeated(None, 64);
    for placed in pieces { // y: #placed, else #no_pieces
        list::set_number(&mut squares, number_of(placed.square), Some(placed.piece));
    }
    Game { squares, to_move, level: levels::ALL[2], turns: 0, quiet: 0, last: None }
}

/// A man of a player's on a square.
///
/// - `owner`: the player.
/// - `row`: the square's row.
/// - `column`: the square's column.
/// - Returns the man and his square.
fn man(owner: Side, row: i32, column: i32) -> Placed {
    Placed { square: Square { row, column }, piece: Piece { owner, king: false } }
}

/// A king of a player's on a square.
///
/// - `owner`: the player.
/// - `row`: the square's row.
/// - `column`: the square's column.
/// - Returns the king and his square.
fn king(owner: Side, row: i32, column: i32) -> Placed {
    Placed { square: Square { row, column }, piece: Piece { owner, king: true } }
}

/// A level that looks at a given number of positions a move, for the tests of the search.
///
/// - `checks`: how many positions.
/// - Returns the level.
fn trial(checks: u32) -> Level {
    Level { name: "Trial", checks, weight: 1 }
}

/// A frame of a fake window in which the player clicks the centre of a square.
///
/// - `row`: the square's row.
/// - `column`: the square's column.
/// - Returns the frame.
fn clicking(row: i32, column: i32) -> Frame {
    Frame { keys: Vec::new(), clicks: vec![centre_of(Square { row, column })] }
}

#[test]
fn a_new_game_has_twelve_men_each() {
    // y: covers new_game > row'many > column'many, starting_piece > placed.light, starting_piece > placed.white_men,
    // starting_piece > placed.red_men'eq, starting_piece > placed.red_men'gt, starting_piece > placed.between,
    // standing > judged.playing, all_steps > placed'many > direction'many > blocked'1,
    // all_steps > placed'many > direction'many > blocked'2, placed_pieces > row'many > column'many > held.piece,
    // placed_pieces > row'many > column'many > held.empty
    let started = new_game(levels::ALL[0]);
    let pieces = placed_pieces(&started);
    assert!(pieces.len() == 24, "twenty_four");
    assert!(piece_at(&started, Square { row: 5, column: 0 }) == Some(Piece { owner: Side::Red, king: false }), "a_red_man");
    assert!(piece_at(&started, Square { row: 2, column: 1 }) == Some(Piece { owner: Side::White, king: false }), "a_white_man");
    assert!(piece_at(&started, Square { row: 0, column: 0 }).is_none(), "light_is_empty");
    assert!(piece_at(&started, Square { row: 3, column: 2 }).is_none(), "the_middle_is_empty");
    assert!(started.to_move == Side::Red, "red_to_move");
    assert!(standing(&started) == Outcome::Playing, "playing");
}

#[test]
fn the_first_moves_are_steps() {
    let mut game = new_game(levels::ALL[0]);
    let reds = legal_moves(&game);
    apply(&mut game, reds[0].clone()); // y: a copy
    let whites = legal_moves(&game);
    assert!(reds.len() == 7, "seven_for_red");
    assert!(whites.len() == 7, "seven_for_white");
    assert!(list::last(&reds[0].path).row == reds[0].from.row - 1, "red_goes_up");
    assert!(list::last(&whites[0].path).row == whites[0].from.row + 1, "white_goes_down");
    assert!(game.to_move == Side::White, "white_to_move");
    assert!(game.quiet == 1, "one_quiet_turn");
}

#[test]
fn a_capture_must_be_taken() {
    // y: covers all_jumps > placed'many, chains_after > uncrowned'2gt
    let mut game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3), man(Side::Red, 5, 6)], Side::Red);
    game.quiet = 7;
    let moves = legal_moves(&game);
    apply(&mut game, moves[0].clone()); // y: a copy
    assert!(moves.len() == 1, "only_the_jump");
    assert!(moves[0].captures == vec![Square { row: 4, column: 3 }], "jumped_over");
    assert!(piece_at(&game, Square { row: 4, column: 3 }).is_none(), "taken");
    assert!(piece_at(&game, Square { row: 3, column: 4 }) == Some(Piece { owner: Side::Red, king: false }), "landed");
    assert!(game.quiet == 0, "no_longer_quiet");
}

#[test]
fn a_chain_of_jumps_is_one_move() {
    // y: covers chains > direction'many > can_jump > goes_on
    let game = arranged(&[man(Side::Red, 6, 1), man(Side::White, 5, 2), man(Side::White, 3, 4)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.len() == 1, "one_move");
    assert!(moves[0].path == vec![Square { row: 4, column: 3 }, Square { row: 2, column: 5 }], "two_landings");
    assert!(moves[0].captures == vec![Square { row: 5, column: 2 }, Square { row: 3, column: 4 }], "two_captures");
}

#[test]
fn a_man_crowned_by_a_jump_stops() {
    // y: covers legal_moves > must_jump, apply > captured, chains > direction'many > can_jump > ends_here,
    // chains_after > crowned, belongs_to > held.piece, far_row > heading_up
    let mut game = arranged(&[man(Side::Red, 2, 1), man(Side::White, 1, 2), man(Side::White, 1, 4)], Side::Red);
    let moves = legal_moves(&game);
    apply(&mut game, moves[0].clone()); // y: a copy
    assert!(moves[0].path == vec![Square { row: 0, column: 3 }], "one_jump");
    assert!(piece_at(&game, Square { row: 0, column: 3 }) == Some(Piece { owner: Side::Red, king: true }), "crowned");
}

#[test]
fn a_king_moves_both_ways() {
    // y: covers legal_moves > no_jump, other > current.red, all_steps > placed > direction'many > open,
    // all_jumps > placed, chains > direction'many > cannot_jump'1, jumpable > inside, belongs_to > held.empty,
    // directions_of > moving.king, on_board
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 0, 1)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.len() == 4, "four_ways");
}

#[test]
fn white_is_crowned_on_the_bottom_row() {
    // y: covers apply > quiet, other > current.white, directions_of > moving.white, far_row > heading_down
    let mut game = arranged(&[man(Side::White, 6, 1), man(Side::Red, 0, 7)], Side::White);
    let moves = legal_moves(&game);
    apply(&mut game, moves[0].clone()); // y: a copy
    assert!(piece_at(&game, list::last(&moves[0].path)) == Some(Piece { owner: Side::White, king: true }), "crowned");
    assert!(game.to_move == Side::Red, "red_to_move");
}

#[test]
fn a_win_scores_by_level_and_moves() {
    // y: covers standing > judged.won, score > won, player_moves
    let mut quick = arranged(&[man(Side::Red, 4, 3)], Side::White);
    quick.turns = 59;
    let mut slow = arranged(&[man(Side::Red, 4, 3)], Side::White);
    slow.turns = 299;
    let quick_points = score(&quick);
    let slow_points = score(&slow);
    assert!(standing(&quick) == Outcome::Won, "won");
    assert!(quick_points == 2100, "quick_score: Expected 1000 points, less 300 for 30 moves, times 3 at the Club level.");
    assert!(slow_points == 300, "slow_score: Expected the least win, 100 points, times 3.");
}

#[test]
fn a_loss_scores_nothing() {
    // y: covers standing > judged.lost, score > unfinished, all_steps > no_pieces, all_jumps > no_pieces,
    // own_pieces > placed > theirs
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let reached = standing(&game);
    let points = score(&game);
    assert!(reached == Outcome::Lost, "lost");
    assert!(points == 0, "nothing");
}

#[test]
fn quiet_turns_draw_the_game() {
    // y: covers standing > judged.drawn'eq, standing > judged.drawn'gt
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let mut later = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    later.quiet = game_rules::QUIET_LIMIT + 1;
    let reached = standing(&game);
    let reached_later = standing(&later);
    assert!(reached == Outcome::Drawn, "drawn");
    assert!(reached_later == Outcome::Drawn, "still_drawn");
}

#[test]
fn pieces_are_worth_their_rank() {
    // y: covers all_steps > placed'many > direction'many > open, value_for > judged.playing
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::Red, 6, 1), man(Side::White, 1, 2)], Side::Red);
    let for_red = value_for(&game, Side::Red);
    let for_white = value_for(&game, Side::White);
    assert!(for_red == 5, "red_ahead: Expected 5 for the king and 3 for the man, less 3 for white's man.");
    assert!(for_white == -5, "white_behind");
}

#[test]
fn an_ended_game_is_worth_its_end() {
    // y: covers value_for > judged.losing, value_for > judged.winning, value_for > judged.drawn'eq,
    // value_for > judged.drawn'gt
    let stuck = arranged(&[man(Side::Red, 4, 3)], Side::White);
    let mut drawn = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    drawn.quiet = game_rules::QUIET_LIMIT;
    let mut later = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    later.quiet = game_rules::QUIET_LIMIT + 1;
    let for_white = value_for(&stuck, Side::White);
    let for_red = value_for(&stuck, Side::Red);
    let for_either = value_for(&drawn, Side::Red);
    let for_either_later = value_for(&later, Side::Red);
    assert!(for_white == -search::WIN_VALUE, "white_lost");
    assert!(for_red == search::WIN_VALUE, "red_won");
    assert!(for_either == 0, "nothing_for_a_draw");
    assert!(for_either_later == 0, "nothing_later");
}

#[test]
fn the_computer_keeps_its_man_out_of_reach() {
    // y: covers computer_move > deepening'many'1lt > candidate'many > finished,
    // computer_move > deepening'many'1lt > candidate'many > ran_out, searched > judged.leaf'2,
    // searched > judged.spent'gt, searched > judged.inner > candidate > others_turn,
    // searched > judged.inner > candidate'many > owners_turn, searched > judged.inner > candidate'many > others_turn,
    // best_sampled > option'many > ranked.top > ties.settled, best_sampled > option'many > ranked.lower > ties.settled
    let mut random = Random::seeded(7);
    let mut game = arranged(&[man(Side::White, 2, 3), man(Side::Red, 4, 5)], Side::White);
    game.level = levels::ALL[1];
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 2 }], "out_of_reach");
}

#[test]
fn the_computer_plays_out_games_to_choose_an_opening() {
    // y: covers apply > captured'many, chains > direction'many > cannot_jump'2, chains_after > uncrowned'1,
    // best_sampled > option'many > ranked.lower > ties.contested > option'many
    let mut random = Random::seeded(7);
    let mut game = new_game(levels::ALL[1]);
    let opening = legal_moves(&game)[0].clone(); // y: a copy
    apply(&mut game, opening);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn without_checks_the_pieces_decide() {
    // y: covers sampled_value > no_share, material > placed'many > counted.king, material > placed'many > counted.man,
    // material > placed'many > counted.other
    let mut random = Random::seeded(7);
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 1, 2)], Side::Red);
    let worth = sampled_value(&game, Side::Red, 0, &mut random);
    assert!(worth == 2, "the_pieces");
}

#[test]
fn a_click_finds_its_square() {
    // y: covers square_at > inside, square_at > outside'1, square_at > outside'2
    let on = square_at(Point { x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5 });
    let off = square_at(Point { x: 5, y: 5 });
    let below = square_at(Point { x: 100, y: 700 });
    assert!(on == Some(Square { row: 5, column: 3 }), "found");
    assert!(off.is_none(), "none");
    assert!(below.is_none(), "none_below");
}

#[test]
fn a_click_finds_its_level() {
    // y: covers level_at > level'many > pressed, level_at > level'many > missed'2
    let on = level_at(Point { x: 100, y: layout::BUTTON_TOP + layout::BUTTON_STEP + 10 });
    let off = level_at(Point { x: 100, y: 10 });
    assert!(on == Some(levels::ALL[1]), "casual");
    assert!(off.is_none(), "none");
}

#[test]
fn squares_are_shaded() {
    // y: covers shade_of > looks.light, shade_of > looks.selected, shade_of > looks.last, shade_of > looks.dark,
    // in_last_move > made.some
    let fresh = new_game(levels::ALL[0]);
    let mut moved = new_game(levels::ALL[0]);
    let opening = legal_moves(&moved)[0].clone(); // y: a copy
    apply(&mut moved, opening);
    let from = moved.last.as_ref().expect("given").from;
    let selected = Some(Square { row: 2, column: 1 });
    let light = shade_of(&fresh, Square { row: 0, column: 0 }, selected);
    let chosen = shade_of(&fresh, Square { row: 2, column: 1 }, selected);
    let plain = shade_of(&fresh, from, selected);
    let last = shade_of(&moved, from, selected);
    assert!(light == palette::LIGHT, "light_square");
    assert!(chosen == palette::SELECTED, "selected_square");
    assert!(plain == palette::DARK, "dark_square");
    assert!(last == palette::LAST, "last_move");
}

#[test]
fn the_selected_piece_shows_where_it_may_go() {
    // y: covers landings > candidate'many > from_selected, landings > candidate'many > from_elsewhere
    let game = new_game(levels::ALL[0]);
    let some = landings(&game, Some(Square { row: 5, column: 2 }));
    let none = landings(&game, None);
    assert!(some == vec![Square { row: 4, column: 1 }, Square { row: 4, column: 3 }], "two_squares");
    assert!(none.is_empty(), "nothing");
}

#[test]
fn pieces_are_painted_by_player_and_rank() {
    // y: covers colours_of > painted.red_king, colours_of > painted.red_man, colours_of > painted.white_king,
    // colours_of > painted.white_man
    let red_king = colours_of(Piece { owner: Side::Red, king: true });
    let red_man = colours_of(Piece { owner: Side::Red, king: false });
    let white_king = colours_of(Piece { owner: Side::White, king: true });
    let white_man = colours_of(Piece { owner: Side::White, king: false });
    assert!(red_king == vec![palette::RED, palette::RED_RING, palette::CROWN], "crowned_red");
    assert!(red_man == vec![palette::RED, palette::RED_RING, palette::RED], "plain_red");
    assert!(white_king == vec![palette::WHITE, palette::WHITE_RING, palette::CROWN], "crowned_white");
    assert!(white_man == vec![palette::WHITE, palette::WHITE_RING, palette::WHITE], "plain_white");
}

#[test]
fn the_banner_says_how_the_game_stands() {
    // y: covers banner > shown.your_move, banner > shown.thinking, banner > shown.won, banner > shown.lost,
    // banner > shown.drawn
    let fresh = new_game(levels::ALL[2]);
    let mut thinking = new_game(levels::ALL[2]);
    let opening = legal_moves(&thinking)[0].clone(); // y: a copy
    apply(&mut thinking, opening);
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let lost = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let mut drawn = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    drawn.quiet = game_rules::QUIET_LIMIT;
    let fresh_lines = banner(&fresh, 50);
    let thinking_lines = banner(&thinking, 50);
    let won_lines = banner(&won, 2100);
    let lost_lines = banner(&lost, 50);
    let drawn_lines = banner(&drawn, 50);
    assert!(fresh_lines[0] == "Your move   Club   Moves 0   Best 50", "says_your_move");
    assert!(thinking_lines[0] == "The computer is thinking, at 2500 positions a move...", "says_thinking");
    assert!(won_lines[0] == "You won in 30 moves against Club! Score 2100   Best 2100", "says_won");
    assert!(lost_lines[0] == "The computer won. Score 0   Best 50", "says_lost");
    assert!(drawn_lines[0] == "A draw: 80 turns without a capture. Score 0   Best 50", "says_drawn");
}

#[test]
fn a_click_elsewhere_selects_nothing() {
    // y: covers centre_of, clicked > judged.elsewhere, move_to > candidate'many > differs'1,
    // move_to > candidate'many > differs'2, may_move_from > candidate'many > unmovable
    let mut game = new_game(levels::ALL[0]);
    let mut selected = Some(Square { row: 5, column: 2 });
    clicked(&mut game, &mut selected, centre_of(Square { row: 0, column: 0 }));
    assert!(selected.is_none(), "nothing_selected");
    assert!(game.to_move == Side::Red, "red_still_to_move");
}

#[test]
fn escape_at_the_levels_ends_play() {
    // y: covers draw_levels > level'many, choose_level > choosing, level_clicked > no_clicks
    let mut window = Window::fake(&[&[Key::Escape]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "Checkers"), "levels_shown");
}

#[test]
fn a_level_is_chosen_and_a_move_made() {
    // y: covers playout > ended'1, draw_marks > landing'many, play > open > picked.level, choose_level > choosing'many,
    // play_game > playing'many > moment.computers_turn, play_game > playing'many > moment.players_turn,
    // level_clicked > place > miss, act_on_clicks > place
    let mut window = Window::fake_frames(vec![
        Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: 10 }] },
        Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + 10 }] },
        clicking(5, 2),
        clicking(4, 3),
        Frame::default(),
        Frame { keys: vec![Key::Escape], clicks: Vec::new() },
    ]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    let written = window.written();
    assert!(best == 0, "no_score");
    assert!(written.iter().any(|line| line == "The computer is thinking, at 25 positions a move..."), "thinking");
    assert!(written.iter().any(|line| line == "Your move   Beginner   Moves 1   Best 0"), "replied");
}

#[test]
fn space_after_a_game_chooses_another() {
    // y: covers draw_pieces > placed, draw_piece, play_game > playing > moment.over
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let mut window = Window::fake(&[&[Key::Space]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(won, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 2100, quitting: false }, "scored");
}

#[test]
fn a_blocked_man_has_no_step() {
    // y: covers all_steps > placed > direction'many > blocked'1, all_steps > placed > direction'many > blocked'2,
    // jumpable > outside, directions_of > moving.red
    let game = arranged(&[man(Side::Red, 1, 0), man(Side::White, 0, 1)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.is_empty(), "none");
    assert!(standing(&game) == Outcome::Lost, "lost");
}

#[test]
fn an_empty_board_has_nothing_on_it() {
    // y: covers own_pieces > no_pieces, material > no_pieces, draw, corner_of, in_last_move > made.none,
    // landings > no_moves, draw_board > row'many > column'many, draw_pieces > no_pieces, draw_marks > no_marks,
    // move_to > no_moves, may_move_from > no_moves
    let mut window = Window::fake(&[]);
    let game = arranged(&[], Side::Red);
    let pieces = own_pieces(&game);
    let worth = material(&game, Side::Red);
    let found = move_to(&game, Some(Square { row: 5, column: 0 }), Some(Square { row: 4, column: 1 }));
    let movable = may_move_from(&game, Some(Square { row: 5, column: 0 }));
    let marks = landings(&game, Some(Square { row: 5, column: 0 }));
    draw(&game, None, 0, &mut window);
    assert!(pieces.is_empty(), "no_pieces");
    assert!(worth == 0, "no_worth");
    assert!(found.is_none(), "no_move");
    assert!(!movable, "not_movable");
    assert!(marks.is_empty(), "no_marks");
    assert!(window.written().iter().any(|line| line == "The computer won. Score 0   Best 0"), "banner_written");
}

#[test]
fn the_only_move_is_found() {
    // y: covers own_pieces > placed'many > mine, own_pieces > placed'many > theirs,
    // landings > candidate > from_selected, draw_marks > landing, move_to > candidate > fits,
    // move_to > candidate > differs'1, move_to > candidate > differs'2, may_move_from > candidate > movable,
    // may_move_from > candidate > unmovable
    let mut window = Window::fake(&[]);
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let from = Some(Square { row: 5, column: 0 });
    let fits = move_to(&game, from, Some(Square { row: 4, column: 1 }));
    let elsewhere = move_to(&game, Some(Square { row: 0, column: 0 }), Some(Square { row: 4, column: 1 }));
    let too_far = move_to(&game, from, Some(Square { row: 3, column: 2 }));
    let movable = may_move_from(&game, from);
    let unmovable = may_move_from(&game, Some(Square { row: 6, column: 1 }));
    let marks = landings(&game, from);
    let pieces = own_pieces(&game);
    draw(&game, from, 0, &mut window);
    assert!(fits.is_some(), "found");
    assert!(elsewhere.is_none(), "not_from_there");
    assert!(too_far.is_none(), "not_to_there");
    assert!(movable, "from_its_square");
    assert!(!unmovable, "not_from_another");
    assert!(marks == vec![Square { row: 4, column: 1 }], "one_mark");
    assert!(pieces.len() == 1, "one_of_red");
}

#[test]
fn a_lone_piece_is_worth_its_rank() {
    // y: covers own_pieces > placed > mine, piece_at, number_of, material > placed > counted.king,
    // material > placed > counted.man, material > placed > counted.other
    let lone_king = arranged(&[king(Side::Red, 4, 3)], Side::Red);
    let lone_man = arranged(&[man(Side::Red, 4, 3)], Side::Red);
    let king_for_red = material(&lone_king, Side::Red);
    let king_for_white = material(&lone_king, Side::White);
    let man_for_red = material(&lone_man, Side::Red);
    let pieces = own_pieces(&lone_man);
    assert!(king_for_red == search::KING_VALUE, "a_king");
    assert!(king_for_white == 0, "nothing");
    assert!(man_for_red == search::MAN_VALUE, "a_man");
    assert!(pieces.len() == 1, "one_piece");
}

#[test]
fn one_option_is_chosen_unweighed() {
    // y: covers best_sampled > option > ranked.top > ties.settled
    let mut random = Random::seeded(7);
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let chosen = best_sampled(&game, &options, &[0], 10, &mut random);
    assert!(chosen == options[0], "the_only_one");
}

#[test]
fn with_no_checks_the_search_runs_out_at_once() {
    // y: covers computer_move > deepening > candidate > ran_out, computer_move > deepening > candidate'many > ran_out,
    // searched > judged.leaf'1, best_sampled > option'many > ranked.top > ties.contested > option'many
    let mut random = Random::seeded(7);
    let mut one = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    one.level = trial(1);
    let mut two = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    two.level = trial(1);
    let only = computer_move(&one, &mut random);
    let either = computer_move(&two, &mut random);
    assert!(only.path == vec![Square { row: 3, column: 6 }], "the_only_move");
    assert!(legal_moves(&two).contains(&either), "a_white_move");
}

#[test]
fn with_few_checks_the_search_runs_out_partway() {
    // y: covers chains_after > uncrowned'2lt, computer_move > deepening'many'1lt > candidate > finished,
    // computer_move > deepening'many'1lt > candidate > ran_out, sampled_value > some_share > sampling,
    // playout > going'many
    let mut random = Random::seeded(7);
    let mut one = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    one.level = trial(24);
    let mut two = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    two.level = trial(46);
    let only = computer_move(&one, &mut random);
    let either = computer_move(&two, &mut random);
    assert!(only.path == vec![Square { row: 3, column: 6 }], "the_only_move");
    assert!(legal_moves(&two).contains(&either), "a_white_move");
}

#[test]
fn the_search_runs_out_at_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate > ran_out,
    // computer_move > deepening'many'1eq > candidate'many > ran_out
    let mut random = Random::seeded(7);
    let mut one = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    one.level = trial(800);
    let mut two = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    two.level = trial(1500);
    let only = computer_move(&one, &mut random);
    let either = computer_move(&two, &mut random);
    assert!(only.path == vec![Square { row: 3, column: 6 }], "the_only_move");
    assert!(legal_moves(&two).contains(&either), "a_white_move");
}

#[test]
fn the_search_finishes_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate > finished,
    // computer_move > deepening'many'1eq > candidate'many > finished,
    // searched > judged.inner > candidate > owners_turn, sampled_value > some_share > sampling'many
    let mut random = Random::seeded(7);
    let mut one = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    one.level = trial(1100);
    let mut two = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    two.level = trial(2100);
    let only = computer_move(&one, &mut random);
    let either = computer_move(&two, &mut random);
    assert!(only.path == vec![Square { row: 3, column: 6 }], "the_only_move");
    assert!(legal_moves(&two).contains(&either), "a_white_move");
}

#[test]
fn the_search_stops_at_a_draw_and_when_spent() {
    // y: covers searched > judged.leaf'3eq, searched > judged.leaf'3gt, searched > judged.spent'eq
    let mut drawn = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    drawn.quiet = game_rules::QUIET_LIMIT;
    let mut later = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    later.quiet = game_rules::QUIET_LIMIT + 1;
    let open = arranged(&[man(Side::Red, 5, 2), man(Side::White, 2, 7)], Side::Red);
    let mut plenty = 100;
    let mut exact = 2;
    let at_draw = searched(&drawn, Side::Red, 3, &mut plenty);
    let after_draw = searched(&later, Side::Red, 3, &mut plenty);
    let spent = searched(&open, Side::Red, 3, &mut exact);
    assert!(at_draw == 0 && after_draw == 0, "nothing_for_a_draw");
    assert!(exact == 0, "none_left");
    assert!(spent == 0, "given_up");
}

#[test]
fn a_game_played_out_may_end_at_once() {
    // y: covers playout > ended'2, playout > ended'3eq, playout > ended'3gt, playout > going
    let mut random = Random::seeded(7);
    let stuck = arranged(&[man(Side::Red, 1, 0), man(Side::White, 0, 1)], Side::Red);
    let mut drawn = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    drawn.quiet = game_rules::QUIET_LIMIT;
    let mut later = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    later.quiet = game_rules::QUIET_LIMIT + 1;
    let open = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    let no_moves = playout(&stuck, Side::Red, 5, &mut random);
    let at_draw = playout(&drawn, Side::Red, 5, &mut random);
    let after_draw = playout(&later, Side::Red, 5, &mut random);
    let one_move = playout(&open, Side::Red, 1, &mut random);
    assert!(no_moves == PlayedOut { value: -search::WIN_VALUE, spent: 1 }, "lost_at_once");
    assert!(at_draw.spent == 1 && after_draw.spent == 1, "drawn_at_once");
    assert!(one_move.spent == 2, "two_positions");
}

#[test]
fn clicks_beside_and_between_the_buttons() {
    // y: covers level_at > level > pressed, level_at > level'many > missed'1, level_clicked > place'many > hit,
    // level_clicked > place'many > miss
    let beside = level_at(Point { x: 595, y: layout::BUTTON_TOP + 10 });
    let second = level_clicked(&[Point { x: 100, y: 10 }, Point { x: 100, y: layout::BUTTON_TOP + 10 }]);
    let neither = level_clicked(&[Point { x: 100, y: 10 }, Point { x: 595, y: 10 }]);
    assert!(beside.is_none(), "none_beside");
    assert!(second == Some(levels::ALL[0]), "the_second");
    assert!(neither.is_none(), "none");
}

#[test]
fn two_clicks_in_a_frame_make_a_move() {
    // y: covers clicked > judged.moving, clicked > judged.choosing, act_on_clicks > place'many,
    // move_to > candidate'many > fits, may_move_from > candidate'many > movable
    let mut game = new_game(levels::ALL[0]);
    let mut selected = None;
    act_on_clicks(&mut game, &mut selected, &[centre_of(Square { row: 5, column: 2 }), centre_of(Square { row: 4, column: 3 })]);
    assert!(piece_at(&game, Square { row: 4, column: 3 }) == Some(Piece { owner: Side::Red, king: false }), "moved");
    assert!(game.to_move == Side::White, "white_to_move");
}

#[test]
fn a_window_closed_at_once_ends_play() {
    // y: covers play > open > picked.none, choose_level > closed_at_once'3
    let mut window = Window::fake(&[]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
}

#[test]
fn a_window_closed_as_a_game_starts_ends_play() {
    // y: covers play > open'many > picked.none, play > open'many > picked.level, play_game > closed_at_once'3,
    // level_clicked > place > hit
    let mut window = Window::fake_frames(vec![Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + 10 }] }]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
}

#[test]
fn the_computer_moves_in_a_frame() {
    // y: covers play_game > playing > moment.computers_turn
    let mut window = Window::fake_frames(vec![Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let computers = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    let after_computer = y_window::block_on(play_game(computers, 0, &mut window, &mut random, &forever));
    assert!(after_computer == Ending { points: 0, quitting: false }, "not_quitting");
}

#[test]
fn the_player_moves_in_a_frame() {
    // y: covers landings > candidate > from_elsewhere, draw_pieces > placed'many,
    // play_game > playing > moment.players_turn, act_on_clicks > no_clicks
    let mut window = Window::fake_frames(vec![Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let players = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::Red);
    let ended = y_window::block_on(play_game(players, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: false }, "not_quitting");
}

#[test]
fn an_ended_game_waits_for_space() {
    // y: covers play_game > playing'many > moment.over
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let mut window = Window::fake(&[&[], &[Key::Space]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(won, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 2100, quitting: false }, "scored");
}
