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
    // y: covers new_game > row'many > column'many
    let started = new_game(levels::ALL[0]);
    assert!(placed_pieces(&started).len() == 24, "twenty_four");
    assert!(piece_at(&started, Square { row: 5, column: 0 }) == Some(Piece { owner: Side::Red, king: false }), "a_red_man");
    assert!(piece_at(&started, Square { row: 2, column: 1 }) == Some(Piece { owner: Side::White, king: false }), "a_white_man");
    assert!(started.to_move == Side::Red, "red_to_move");
    assert!(started.turns == 0, "no_turns_yet");
    assert!(started.level == levels::ALL[0], "at_the_level");
}

#[test]
fn a_light_square_starts_empty() {
    // y: covers starting_piece > placed.light
    let held = starting_piece(Square { row: 0, column: 0 });
    assert!(held.is_none(), "empty");
}

#[test]
fn the_top_rows_start_with_white_men() {
    // y: covers starting_piece > placed.white_men
    let held = starting_piece(Square { row: 2, column: 1 });
    assert!(held == Some(Piece { owner: Side::White, king: false }), "a_man");
}

#[test]
fn the_first_of_the_bottom_rows_starts_with_a_red_man() {
    // y: covers starting_piece > placed.red_men'eq
    let held = starting_piece(Square { row: 5, column: 0 });
    assert!(held == Some(Piece { owner: Side::Red, king: false }), "a_man");
}

#[test]
fn the_bottom_row_starts_with_a_red_man() {
    // y: covers starting_piece > placed.red_men'gt
    let held = starting_piece(Square { row: 7, column: 0 });
    assert!(held == Some(Piece { owner: Side::Red, king: false }), "a_man");
}

#[test]
fn the_middle_rows_start_empty() {
    // y: covers starting_piece > placed.between
    let held = starting_piece(Square { row: 3, column: 0 });
    assert!(held.is_none(), "empty");
}

#[test]
fn a_capture_must_be_taken() {
    // y: covers legal_moves > must_jump
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3), man(Side::Red, 5, 6)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.len() == 1, "only_the_jump");
    assert!(moves[0].captures == vec![Square { row: 4, column: 3 }], "jumped_over");
}

#[test]
fn the_first_moves_are_steps() {
    // y: covers legal_moves > no_jump
    let game = new_game(levels::ALL[0]);
    let moves = legal_moves(&game);
    assert!(moves.len() == 7, "seven");
    assert!(list::last(&moves[0].path).row == moves[0].from.row - 1, "red_goes_up");
    assert!(moves[0].captures.is_empty(), "no_captures");
}

#[test]
fn a_step_is_a_quiet_turn() {
    // y: covers apply > quiet
    let mut game = new_game(levels::ALL[0]);
    let step = Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 0 }, path: vec![Square { row: 4, column: 1 }], captures: Vec::new() };
    apply(&mut game, step);
    assert!(piece_at(&game, Square { row: 5, column: 0 }).is_none(), "left");
    assert!(piece_at(&game, Square { row: 4, column: 1 }) == Some(Piece { owner: Side::Red, king: false }), "arrived");
    assert!(game.quiet == 1, "one_quiet_turn");
    assert!(game.turns == 1, "one_turn");
    assert!(game.to_move == Side::White, "white_to_move");
    assert!(game.last.is_some(), "remembered");
}

#[test]
fn a_white_man_stepping_onto_the_bottom_row_is_crowned() {
    // y: covers apply > quiet
    let mut game = arranged(&[man(Side::White, 6, 1), man(Side::Red, 0, 7)], Side::White);
    let step = Move { piece: Piece { owner: Side::White, king: false }, from: Square { row: 6, column: 1 }, path: vec![Square { row: 7, column: 0 }], captures: Vec::new() };
    apply(&mut game, step);
    assert!(piece_at(&game, Square { row: 7, column: 0 }) == Some(Piece { owner: Side::White, king: true }), "crowned");
    assert!(game.to_move == Side::Red, "red_to_move");
}

#[test]
fn a_jump_takes_the_piece_jumped_over() {
    // y: covers apply > captured
    let mut game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3)], Side::Red);
    game.quiet = 7;
    let jump = Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 2 }, path: vec![Square { row: 3, column: 4 }], captures: vec![Square { row: 4, column: 3 }] };
    apply(&mut game, jump);
    assert!(piece_at(&game, Square { row: 4, column: 3 }).is_none(), "taken");
    assert!(piece_at(&game, Square { row: 3, column: 4 }) == Some(Piece { owner: Side::Red, king: false }), "landed");
    assert!(game.quiet == 0, "no_longer_quiet");
}

#[test]
fn a_man_crowned_by_a_jump() {
    // y: covers apply > captured
    let mut game = arranged(&[man(Side::Red, 2, 1), man(Side::White, 1, 2)], Side::Red);
    let jump = Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 2, column: 1 }, path: vec![Square { row: 0, column: 3 }], captures: vec![Square { row: 1, column: 2 }] };
    apply(&mut game, jump);
    assert!(piece_at(&game, Square { row: 0, column: 3 }) == Some(Piece { owner: Side::Red, king: true }), "crowned");
}

#[test]
fn a_chain_of_jumps_takes_every_piece_jumped() {
    // y: covers apply > captured'many
    let mut game = arranged(&[man(Side::Red, 6, 1), man(Side::White, 5, 2), man(Side::White, 3, 4)], Side::Red);
    let chain = Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 6, column: 1 }, path: vec![Square { row: 4, column: 3 }, Square { row: 2, column: 5 }], captures: vec![Square { row: 5, column: 2 }, Square { row: 3, column: 4 }] };
    apply(&mut game, chain);
    assert!(piece_at(&game, Square { row: 5, column: 2 }).is_none(), "first_taken");
    assert!(piece_at(&game, Square { row: 3, column: 4 }).is_none(), "second_taken");
    assert!(piece_at(&game, Square { row: 2, column: 5 }) == Some(Piece { owner: Side::Red, king: false }), "landed");
    assert!(game.turns == 1, "one_turn");
}

#[test]
fn the_person_wins_when_the_computer_cannot_move() {
    // y: covers standing > judged.won
    let game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    let reached = standing(&game);
    assert!(reached == Outcome::Won, "won");
}

#[test]
fn the_person_loses_when_they_cannot_move() {
    // y: covers standing > judged.lost
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let reached = standing(&game);
    assert!(reached == Outcome::Lost, "lost");
}

#[test]
fn quiet_turns_draw_the_game() {
    // y: covers standing > judged.drawn'eq
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let reached = standing(&game);
    assert!(reached == Outcome::Drawn, "drawn");
}

#[test]
fn more_quiet_turns_still_draw_the_game() {
    // y: covers standing > judged.drawn'gt
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT + 1;
    let reached = standing(&game);
    assert!(reached == Outcome::Drawn, "drawn");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers standing > judged.playing
    let game = new_game(levels::ALL[0]);
    let reached = standing(&game);
    assert!(reached == Outcome::Playing, "playing");
}

#[test]
fn a_win_scores_by_level_and_moves() {
    // y: covers score > won
    let mut game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    game.turns = 59;
    let points = score(&game);
    assert!(points == 2100, "scored: Expected 1000 points, less 300 for 30 moves, times 3 at the Club level.");
}

#[test]
fn a_slow_win_scores_the_least_win() {
    // y: covers score > won
    let mut game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    game.turns = 299;
    let points = score(&game);
    assert!(points == 300, "least: Expected the least win, 100 points, times 3.");
}

#[test]
fn a_loss_scores_nothing() {
    // y: covers score > unfinished
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let points = score(&game);
    assert!(points == 0, "nothing");
}

#[test]
fn red_has_made_every_odd_turn() {
    // y: covers player_moves
    let mut game = new_game(levels::ALL[0]);
    game.turns = 59;
    let made = player_moves(&game);
    assert!(made == 30, "thirty");
}

#[test]
fn white_moves_after_red() {
    // y: covers other > current.red
    let next = other(Side::Red);
    assert!(next == Side::White, "white");
}

#[test]
fn red_moves_after_white() {
    // y: covers other > current.white
    let next = other(Side::White);
    assert!(next == Side::Red, "red");
}

#[test]
fn a_player_with_no_pieces_has_no_steps() {
    // y: covers all_steps > no_pieces
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.is_empty(), "none");
}

#[test]
fn a_lone_man_steps_both_ways_ahead() {
    // y: covers all_steps > placed > direction'many > open
    let game = arranged(&[man(Side::Red, 5, 2)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.len() == 2, "two_steps");
    assert!(steps[0].path == vec![Square { row: 4, column: 1 }], "up_left");
    assert!(steps[1].path == vec![Square { row: 4, column: 3 }], "up_right");
}

#[test]
fn a_man_on_the_edge_steps_only_inward() {
    // y: covers all_steps > placed > direction'many > blocked'1
    let game = arranged(&[man(Side::Red, 5, 0)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.len() == 1, "one_step");
    assert!(steps[0].path == vec![Square { row: 4, column: 1 }], "inward");
}

#[test]
fn a_man_with_both_squares_ahead_taken_has_no_step() {
    // y: covers all_steps > placed > direction'many > blocked'2
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 1), man(Side::White, 4, 3)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.is_empty(), "none");
}

#[test]
fn each_of_several_men_steps() {
    // y: covers all_steps > placed'many > direction'many > open
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::Red, 7, 6)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.len() == 4, "four_steps");
    assert!(steps[0].from == Square { row: 5, column: 2 }, "the_upper_man_first");
    assert!(steps[3].from == Square { row: 7, column: 6 }, "the_lower_man_last");
}

#[test]
fn men_on_the_edge_step_only_inward() {
    // y: covers all_steps > placed'many > direction'many > blocked'1
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::Red, 7, 0)], Side::Red);
    let steps = all_steps(&game);
    assert!(steps.len() == 2, "two_steps");
    assert!(steps[0].path == vec![Square { row: 4, column: 1 }], "the_upper_inward");
    assert!(steps[1].path == vec![Square { row: 6, column: 1 }], "the_lower_inward");
}

#[test]
fn men_behind_the_front_row_are_blocked_at_the_start() {
    // y: covers all_steps > placed'many > direction'many > blocked'2
    let game = new_game(levels::ALL[0]);
    let steps = all_steps(&game);
    assert!(steps.len() == 7, "seven");
    assert!(steps[6].from.row == 5, "from_the_front_row");
}

#[test]
fn a_player_with_no_pieces_has_no_jumps() {
    // y: covers all_jumps > no_pieces
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let jumps = all_jumps(&game);
    assert!(jumps.is_empty(), "none");
}

#[test]
fn a_lone_man_jumps_the_piece_ahead() {
    // y: covers all_jumps > placed
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3)], Side::Red);
    let jumps = all_jumps(&game);
    assert!(jumps.len() == 1, "one_jump");
    assert!(jumps[0].path == vec![Square { row: 3, column: 4 }], "landed_beyond");
}

#[test]
fn only_the_men_that_can_jump_have_jumps() {
    // y: covers all_jumps > placed'many
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3), man(Side::Red, 5, 6)], Side::Red);
    let jumps = all_jumps(&game);
    assert!(jumps.len() == 1, "one_jump");
    assert!(jumps[0].from == Square { row: 5, column: 2 }, "from_the_jumper");
}

#[test]
fn a_jump_with_none_after_it_ends_the_chain() {
    // y: covers chains > direction'many > can_jump > ends_here
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3)], Side::Red);
    let found = chains(&game, Piece { owner: Side::Red, king: false }, Square { row: 5, column: 2 }, Square { row: 5, column: 2 }, &[], &[]);
    assert!(found.len() == 1, "one_move");
    assert!(found[0].path == vec![Square { row: 3, column: 4 }], "one_landing");
    assert!(found[0].captures == vec![Square { row: 4, column: 3 }], "one_capture");
}

#[test]
fn a_chain_of_jumps_is_one_move() {
    // y: covers chains > direction'many > can_jump > goes_on
    let game = arranged(&[man(Side::Red, 6, 1), man(Side::White, 5, 2), man(Side::White, 3, 4)], Side::Red);
    let found = chains(&game, Piece { owner: Side::Red, king: false }, Square { row: 6, column: 1 }, Square { row: 6, column: 1 }, &[], &[]);
    assert!(found.len() == 1, "one_move");
    assert!(found[0].path == vec![Square { row: 4, column: 3 }, Square { row: 2, column: 5 }], "two_landings");
    assert!(found[0].captures == vec![Square { row: 5, column: 2 }, Square { row: 3, column: 4 }], "two_captures");
}

#[test]
fn a_man_with_nothing_to_jump_has_no_chain() {
    // y: covers chains > direction'many > cannot_jump'1
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 2, 7)], Side::Red);
    let found = chains(&game, Piece { owner: Side::Red, king: false }, Square { row: 5, column: 2 }, Square { row: 5, column: 2 }, &[], &[]);
    assert!(found.is_empty(), "none");
}

#[test]
fn a_piece_is_not_jumped_twice() {
    // y: covers chains > direction'many > cannot_jump'2
    let game = arranged(&[king(Side::Red, 5, 2), man(Side::White, 4, 3)], Side::Red);
    let found = chains(&game, Piece { owner: Side::Red, king: true }, Square { row: 5, column: 2 }, Square { row: 3, column: 4 }, &[Square { row: 3, column: 4 }], &[Square { row: 4, column: 3 }]);
    assert!(found.is_empty(), "no_further_jump");
}

#[test]
fn a_man_crowned_by_a_jump_stops() {
    // y: covers chains_after > crowned
    let game = arranged(&[man(Side::Red, 2, 1), man(Side::White, 1, 2), man(Side::White, 1, 4)], Side::Red);
    let found = chains_after(&game, Piece { owner: Side::Red, king: false }, Square { row: 2, column: 1 }, Square { row: 0, column: 3 }, &[Square { row: 0, column: 3 }], &[Square { row: 1, column: 2 }]);
    assert!(found.is_empty(), "ends_there");
}

#[test]
fn a_king_on_the_far_row_jumps_on() {
    // y: covers chains_after > uncrowned'1
    let game = arranged(&[king(Side::Red, 2, 1), man(Side::White, 1, 2), man(Side::White, 1, 4)], Side::Red);
    let found = chains_after(&game, Piece { owner: Side::Red, king: true }, Square { row: 2, column: 1 }, Square { row: 0, column: 3 }, &[Square { row: 0, column: 3 }], &[Square { row: 1, column: 2 }]);
    assert!(found.len() == 1, "one_more_jump");
    assert!(found[0].path == vec![Square { row: 0, column: 3 }, Square { row: 2, column: 5 }], "on_to_the_next");
}

#[test]
fn a_white_man_short_of_the_bottom_row_jumps_on() {
    // y: covers chains_after > uncrowned'2lt
    let game = arranged(&[man(Side::White, 1, 2), man(Side::Red, 2, 3), man(Side::Red, 4, 5)], Side::White);
    let found = chains_after(&game, Piece { owner: Side::White, king: false }, Square { row: 1, column: 2 }, Square { row: 3, column: 4 }, &[Square { row: 3, column: 4 }], &[Square { row: 2, column: 3 }]);
    assert!(found.len() == 1, "one_more_jump");
    assert!(found[0].path == vec![Square { row: 3, column: 4 }, Square { row: 5, column: 6 }], "on_to_the_next");
}

#[test]
fn a_red_man_short_of_the_top_row_jumps_on() {
    // y: covers chains_after > uncrowned'2gt
    let game = arranged(&[man(Side::Red, 6, 1), man(Side::White, 5, 2), man(Side::White, 3, 4)], Side::Red);
    let found = chains_after(&game, Piece { owner: Side::Red, king: false }, Square { row: 6, column: 1 }, Square { row: 4, column: 3 }, &[Square { row: 4, column: 3 }], &[Square { row: 5, column: 2 }]);
    assert!(found.len() == 1, "one_more_jump");
    assert!(found[0].path == vec![Square { row: 4, column: 3 }, Square { row: 2, column: 5 }], "on_to_the_next");
}

#[test]
fn a_piece_may_jump_onto_an_empty_square() {
    // y: covers jumpable > inside
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 4, 3)], Side::Red);
    let can = jumpable(&game, Piece { owner: Side::Red, king: false }, Square { row: 5, column: 2 }, Square { row: 4, column: 3 }, Square { row: 3, column: 4 });
    assert!(can, "it_may");
}

#[test]
fn a_piece_may_not_jump_off_the_board() {
    // y: covers jumpable > outside
    let game = arranged(&[man(Side::Red, 1, 2), man(Side::White, 0, 3)], Side::Red);
    let can = jumpable(&game, Piece { owner: Side::Red, king: false }, Square { row: 1, column: 2 }, Square { row: 0, column: 3 }, Square { row: -1, column: 4 });
    assert!(!can, "it_may_not");
}

#[test]
fn an_empty_square_belongs_to_no_one() {
    // y: covers belongs_to > held.empty
    let game = arranged(&[man(Side::White, 4, 3)], Side::Red);
    let holds = belongs_to(&game, Square { row: 3, column: 4 }, Side::White);
    assert!(!holds, "not_white");
}

#[test]
fn a_square_belongs_to_the_owner_of_its_piece() {
    // y: covers belongs_to > held.piece
    let game = arranged(&[man(Side::White, 4, 3)], Side::Red);
    let holds = belongs_to(&game, Square { row: 4, column: 3 }, Side::White);
    assert!(holds, "white");
}

#[test]
fn every_piece_is_found_with_its_square() {
    // y: covers placed_pieces > row'many > column'many > held.piece
    let game = new_game(levels::ALL[0]);
    let found = placed_pieces(&game);
    assert!(found.len() == 24, "twenty_four");
    assert!(found[0] == man(Side::White, 0, 1), "a_white_man_first");
    assert!(found[23] == man(Side::Red, 7, 6), "a_red_man_last");
}

#[test]
fn an_empty_board_has_no_pieces() {
    // y: covers placed_pieces > row'many > column'many > held.empty
    let game = arranged(&[], Side::Red);
    let found = placed_pieces(&game);
    assert!(found.is_empty(), "none");
}

#[test]
fn a_player_on_an_empty_board_has_no_pieces() {
    // y: covers own_pieces > no_pieces
    let game = arranged(&[], Side::Red);
    let found = own_pieces(&game);
    assert!(found.is_empty(), "none");
}

#[test]
fn a_lone_piece_is_its_players_own() {
    // y: covers own_pieces > placed > mine
    let game = arranged(&[man(Side::Red, 4, 3)], Side::Red);
    let found = own_pieces(&game);
    assert!(found == vec![man(Side::Red, 4, 3)], "the_one");
}

#[test]
fn a_lone_piece_of_the_other_player_is_not_own() {
    // y: covers own_pieces > placed > theirs
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let found = own_pieces(&game);
    assert!(found.is_empty(), "none");
}

#[test]
fn several_pieces_of_the_player_are_own() {
    // y: covers own_pieces > placed'many > mine
    let game = arranged(&[man(Side::Red, 5, 0), king(Side::Red, 5, 2)], Side::Red);
    let found = own_pieces(&game);
    assert!(found == vec![man(Side::Red, 5, 0), king(Side::Red, 5, 2)], "both");
}

#[test]
fn the_other_players_pieces_are_left_out() {
    // y: covers own_pieces > placed'many > theirs
    let game = arranged(&[man(Side::White, 2, 1), man(Side::Red, 5, 0)], Side::Red);
    let found = own_pieces(&game);
    assert!(found == vec![man(Side::Red, 5, 0)], "only_red");
}

#[test]
fn a_king_moves_every_way() {
    // y: covers directions_of > moving.king
    let found = directions_of(Piece { owner: Side::Red, king: true });
    assert!(found == directions::ALL, "all");
}

#[test]
fn a_red_man_moves_up() {
    // y: covers directions_of > moving.red
    let found = directions_of(Piece { owner: Side::Red, king: false });
    assert!(found == directions::UP, "up");
}

#[test]
fn a_white_man_moves_down() {
    // y: covers directions_of > moving.white
    let found = directions_of(Piece { owner: Side::White, king: false });
    assert!(found == directions::DOWN, "down");
}

#[test]
fn red_is_crowned_on_the_top_row() {
    // y: covers far_row > heading_up
    let row = far_row(Side::Red);
    assert!(row == 0, "top");
}

#[test]
fn white_is_crowned_on_the_bottom_row() {
    // y: covers far_row > heading_down
    let row = far_row(Side::White);
    assert!(row == 7, "bottom");
}

#[test]
fn the_piece_on_a_square_is_found() {
    // y: covers piece_at
    let game = arranged(&[king(Side::White, 4, 3)], Side::Red);
    let held = piece_at(&game, Square { row: 4, column: 3 });
    assert!(held == Some(Piece { owner: Side::White, king: true }), "a_white_king");
}

#[test]
fn squares_are_numbered_row_by_row() {
    // y: covers number_of
    let number = number_of(Square { row: 1, column: 2 });
    assert!(number == 11, "eleventh");
}

#[test]
fn a_corner_is_on_the_board() {
    // y: covers on_board
    let inside = on_board(Square { row: 7, column: 7 });
    assert!(inside, "on");
}

#[test]
fn with_one_move_and_no_checks_the_search_runs_out_at_once() {
    // y: covers computer_move > deepening > candidate > ran_out
    let mut game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(1);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 6 }], "the_only_move");
}

#[test]
fn with_two_moves_and_no_checks_the_search_runs_out_at_once() {
    // y: covers computer_move > deepening > candidate'many > ran_out
    let mut game = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(1);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn with_one_move_and_two_checks_the_search_finishes_a_depth() {
    // y: covers computer_move > deepening'many'1lt > candidate > finished
    let mut game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(2);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 6 }], "the_only_move");
}

#[test]
fn with_one_move_and_a_few_checks_the_search_runs_out_partway() {
    // y: covers computer_move > deepening'many'1lt > candidate > ran_out
    let mut game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(24);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 6 }], "the_only_move");
}

#[test]
fn with_two_moves_and_a_few_checks_the_search_finishes_some_depths() {
    // y: covers computer_move > deepening'many'1lt > candidate'many > finished
    let mut game = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(46);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn with_one_move_and_enough_checks_the_search_finishes_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate > finished
    let mut game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(1100);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 6 }], "the_only_move");
}

#[test]
fn with_one_move_the_search_runs_out_at_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate > ran_out
    let mut game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(800);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 6 }], "the_only_move");
}

#[test]
fn with_two_moves_and_enough_checks_the_search_finishes_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate'many > finished
    let mut game = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(2100);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn with_two_moves_the_search_runs_out_at_its_deepest() {
    // y: covers computer_move > deepening'many'1eq > candidate'many > ran_out
    let mut game = arranged(&[man(Side::White, 2, 5), man(Side::Red, 7, 0)], Side::White);
    game.level = trial(1500);
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn the_computer_keeps_its_man_out_of_reach() {
    // y: covers computer_move > deepening'many'1lt > candidate'many > ran_out
    let mut game = arranged(&[man(Side::White, 2, 3), man(Side::Red, 4, 5)], Side::White);
    game.level = levels::ALL[1];
    let mut random = Random::seeded(7);
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 2 }], "out_of_reach");
}

#[test]
fn looking_no_moves_ahead_a_position_is_worth_its_pieces() {
    // y: covers searched > judged.leaf'1
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 1), man(Side::White, 0, 7)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::Red, 0, &mut left);
    assert!(value == -3, "worth");
    assert!(left == 10, "checks_left");
}

#[test]
fn a_position_with_no_moves_is_worth_its_end() {
    // y: covers searched > judged.leaf'2
    let game = arranged(&[man(Side::Red, 1, 0), man(Side::White, 0, 1)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::Red, 3, &mut left);
    assert!(value == -search::WIN_VALUE, "worth");
    assert!(left == 10, "checks_left");
}

#[test]
fn the_search_stops_at_a_draw() {
    // y: covers searched > judged.leaf'3eq
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let mut left = 10;
    let value = searched(&game, Side::Red, 3, &mut left);
    assert!(value == 0, "worth");
    assert!(left == 10, "checks_left");
}

#[test]
fn the_search_stops_past_a_draw() {
    // y: covers searched > judged.leaf'3gt
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT + 1;
    let mut left = 10;
    let value = searched(&game, Side::Red, 3, &mut left);
    assert!(value == 0, "worth");
    assert!(left == 10, "checks_left");
}

#[test]
fn the_search_gives_up_with_exactly_the_checks_the_moves_need() {
    // y: covers searched > judged.spent'eq
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 2, 7)], Side::Red);
    let mut left = 2;
    let value = searched(&game, Side::Red, 3, &mut left);
    assert!(value == 0, "worth");
    assert!(left == 0, "checks_left");
}

#[test]
fn the_search_gives_up_with_fewer_checks_than_moves() {
    // y: covers searched > judged.spent'gt
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 2, 7)], Side::Red);
    let mut left = 1;
    let value = searched(&game, Side::Red, 3, &mut left);
    assert!(value == 0, "worth");
    assert!(left == 0, "checks_left");
}

#[test]
fn the_only_move_of_the_owner_is_weighed() {
    // y: covers searched > judged.inner > candidate > owners_turn
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 1), man(Side::White, 0, 7)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::Red, 1, &mut left);
    assert!(value == -3, "worth");
    assert!(left == 9, "checks_left");
}

#[test]
fn the_only_move_of_the_other_player_is_weighed() {
    // y: covers searched > judged.inner > candidate > others_turn
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 1), man(Side::White, 0, 7)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::White, 1, &mut left);
    assert!(value == 3, "worth");
    assert!(left == 9, "checks_left");
}

#[test]
fn the_owner_takes_the_best_of_its_moves() {
    // y: covers searched > judged.inner > candidate'many > owners_turn
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 3, 0)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::Red, 2, &mut left);
    assert!(value == 0, "even: Expected the step out of reach of white's man, after which the pieces are even.");
    assert!(left < 10, "checks_spent");
}

#[test]
fn the_other_player_takes_the_worst_of_its_moves() {
    // y: covers searched > judged.inner > candidate'many > others_turn
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 3, 0)], Side::Red);
    let mut left = 10;
    let value = searched(&game, Side::White, 2, &mut left);
    assert!(value == 0, "even");
    assert!(left < 10, "checks_spent");
}

#[test]
fn one_option_is_chosen_unweighed() {
    // y: covers best_sampled > option > ranked.top > ties.settled
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let mut random = Random::seeded(7);
    let chosen = best_sampled(&game, &options, &[0], 10, &mut random);
    assert!(chosen == options[0], "the_best");
}

#[test]
fn the_first_of_two_weighed_best_is_chosen() {
    // y: covers best_sampled > option'many > ranked.top > ties.settled
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let mut random = Random::seeded(7);
    let chosen = best_sampled(&game, &options, &[5, 3], 10, &mut random);
    assert!(chosen == options[0], "the_best");
}

#[test]
fn tied_moves_are_weighed_by_games_played_out() {
    // y: covers best_sampled > option'many > ranked.top > ties.contested > option'many
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let mut random = Random::seeded(7);
    let chosen = best_sampled(&game, &options, &[2, 2], 0, &mut random);
    assert!(chosen == options[0], "the_best");
}

#[test]
fn the_second_of_two_weighed_best_is_chosen() {
    // y: covers best_sampled > option'many > ranked.lower > ties.settled
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let mut random = Random::seeded(7);
    let chosen = best_sampled(&game, &options, &[3, 5], 10, &mut random);
    assert!(chosen == options[1], "the_best");
}

#[test]
fn moves_weighed_lower_are_left_out_of_the_games() {
    // y: covers best_sampled > option'many > ranked.lower > ties.contested > option'many
    let game = arranged(&[man(Side::Red, 5, 2), man(Side::Red, 5, 6), man(Side::White, 0, 7)], Side::Red);
    let options = legal_moves(&game);
    let mut random = Random::seeded(7);
    let chosen = best_sampled(&game, &options, &[2, 1, 2, 1], 0, &mut random);
    assert!(chosen == options[0], "the_best");
}

#[test]
fn without_checks_the_pieces_decide() {
    // y: covers sampled_value > no_share
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 1, 2)], Side::Red);
    let mut random = Random::seeded(7);
    let worth = sampled_value(&game, Side::Red, 0, &mut random);
    assert!(worth == 2, "the_pieces");
}

#[test]
fn one_check_plays_one_game_of_no_moves() {
    // y: covers sampled_value > some_share > sampling
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 1, 2)], Side::Red);
    let mut random = Random::seeded(7);
    let worth = sampled_value(&game, Side::Red, 1, &mut random);
    assert!(worth == 2, "the_pieces");
}

#[test]
fn games_are_played_until_the_checks_run_out() {
    // y: covers sampled_value > some_share > sampling'many
    let game = arranged(&[man(Side::Red, 1, 0), man(Side::White, 0, 1)], Side::Red);
    let mut random = Random::seeded(7);
    let worth = sampled_value(&game, Side::Red, 2, &mut random);
    assert!(worth == -search::WIN_VALUE, "lost: Expected two games, each lost at once.");
}

#[test]
fn a_game_played_out_for_no_moves_ends_at_once() {
    // y: covers playout > ended'1
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 1, 2)], Side::Red);
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 0, &mut random);
    assert!(played == PlayedOut { value: 2, spent: 1 }, "ended");
}

#[test]
fn a_game_with_no_moves_ends_at_once() {
    // y: covers playout > ended'2
    let game = arranged(&[man(Side::Red, 1, 0), man(Side::White, 0, 1)], Side::Red);
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 5, &mut random);
    assert!(played == PlayedOut { value: -search::WIN_VALUE, spent: 1 }, "ended");
}

#[test]
fn a_drawn_game_ends_at_once() {
    // y: covers playout > ended'3eq
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 5, &mut random);
    assert!(played == PlayedOut { value: 0, spent: 1 }, "ended");
}

#[test]
fn a_game_past_the_draw_ends_at_once() {
    // y: covers playout > ended'3gt
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT + 1;
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 5, &mut random);
    assert!(played == PlayedOut { value: 0, spent: 1 }, "ended");
}

#[test]
fn a_game_played_out_for_one_move() {
    // y: covers playout > going
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 1, &mut random);
    assert!(played == PlayedOut { value: 0, spent: 2 }, "ended");
}

#[test]
fn a_game_played_out_for_two_moves() {
    // y: covers playout > going'many
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    let mut random = Random::seeded(7);
    let played = playout(&game, Side::Red, 2, &mut random);
    assert!(played == PlayedOut { value: 0, spent: 3 }, "ended");
}

#[test]
fn a_lost_game_is_worth_less_the_win_value() {
    // y: covers value_for > judged.losing
    let game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    let worth = value_for(&game, Side::White);
    assert!(worth == -search::WIN_VALUE, "valued");
}

#[test]
fn a_won_game_is_worth_the_win_value() {
    // y: covers value_for > judged.winning
    let game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    let worth = value_for(&game, Side::Red);
    assert!(worth == search::WIN_VALUE, "valued");
}

#[test]
fn a_drawn_game_is_worth_nothing() {
    // y: covers value_for > judged.drawn'eq
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let worth = value_for(&game, Side::Red);
    assert!(worth == 0, "valued");
}

#[test]
fn a_game_past_the_draw_is_worth_nothing() {
    // y: covers value_for > judged.drawn'gt
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT + 1;
    let worth = value_for(&game, Side::Red);
    assert!(worth == 0, "valued");
}

#[test]
fn pieces_are_worth_their_rank() {
    // y: covers value_for > judged.playing
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::Red, 6, 1), man(Side::White, 1, 2)], Side::Red);
    let worth = value_for(&game, Side::Red);
    assert!(worth == 5, "ahead: Expected 5 for the king and 3 for the man, less 3 for white's man.");
}

#[test]
fn an_empty_board_is_worth_nothing() {
    // y: covers material > no_pieces
    let game = arranged(&[], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == 0, "valued");
}

#[test]
fn a_lone_king_is_worth_a_kings_value() {
    // y: covers material > placed > counted.king
    let game = arranged(&[king(Side::Red, 4, 3)], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == search::KING_VALUE, "valued");
}

#[test]
fn a_lone_man_is_worth_a_mans_value() {
    // y: covers material > placed > counted.man
    let game = arranged(&[man(Side::Red, 4, 3)], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == search::MAN_VALUE, "valued");
}

#[test]
fn a_lone_piece_is_worth_nothing_to_the_other_player() {
    // y: covers material > placed > counted.other
    let game = arranged(&[king(Side::Red, 4, 3)], Side::Red);
    let worth = material(&game, Side::White);
    assert!(worth == 0, "valued");
}

#[test]
fn kings_add_up() {
    // y: covers material > placed'many > counted.king
    let game = arranged(&[king(Side::Red, 4, 3), king(Side::Red, 6, 5)], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == 2 * search::KING_VALUE, "valued");
}

#[test]
fn men_add_up() {
    // y: covers material > placed'many > counted.man
    let game = arranged(&[man(Side::Red, 4, 3), man(Side::Red, 6, 5)], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == 2 * search::MAN_VALUE, "valued");
}

#[test]
fn the_other_players_pieces_add_nothing() {
    // y: covers material > placed'many > counted.other
    let game = arranged(&[man(Side::Red, 4, 3), king(Side::White, 1, 2), man(Side::White, 1, 6)], Side::Red);
    let worth = material(&game, Side::Red);
    assert!(worth == search::MAN_VALUE, "valued");
}

#[test]
fn the_banner_asks_for_the_persons_move() {
    // y: covers banner > shown.your_move
    let game = new_game(levels::ALL[2]);
    let lines = banner(&game, 50);
    assert!(lines == vec!["Your move   Club   Moves 0   Best 50", "Click a red piece, then a marked square. Escape quits."], "says");
}

#[test]
fn the_banner_says_the_computer_is_thinking() {
    // y: covers banner > shown.thinking
    let mut game = new_game(levels::ALL[2]);
    game.to_move = Side::White;
    game.turns = 1;
    let lines = banner(&game, 50);
    assert!(lines == vec!["The computer is thinking, at 2500 positions a move...", "Escape quits."], "says");
}

#[test]
fn the_banner_gives_the_score_of_a_win() {
    // y: covers banner > shown.won
    let mut game = arranged(&[man(Side::Red, 4, 3)], Side::White);
    game.turns = 59;
    let lines = banner(&game, 2100);
    assert!(lines == vec!["You won in 30 moves against Club! Score 2100   Best 2100", "Space chooses a new game. Escape quits."], "says");
}

#[test]
fn the_banner_says_the_computer_won() {
    // y: covers banner > shown.lost
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let lines = banner(&game, 50);
    assert!(lines == vec!["The computer won. Score 0   Best 50", "Space chooses a new game. Escape quits."], "says");
}

#[test]
fn the_banner_says_the_game_is_drawn() {
    // y: covers banner > shown.drawn
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let lines = banner(&game, 50);
    assert!(lines == vec!["A draw: 80 turns without a capture. Score 0   Best 50", "Space chooses a new game. Escape quits."], "says");
}

#[test]
fn the_banner_is_written_beneath_the_board() {
    // y: covers draw
    let game = arranged(&[], Side::Red);
    let mut window = Window::fake(&[]);
    draw(&game, None, 0, &mut window);
    assert!(window.written().iter().any(|line| line == "The computer won. Score 0   Best 0"), "banner_written");
    assert!(window.written().iter().any(|line| line == "Space chooses a new game. Escape quits."), "what_to_do_written");
}

#[test]
fn the_levels_are_drawn_with_the_best_score() {
    // y: covers draw_levels > level'many
    let mut window = Window::fake(&[]);
    draw_levels(2100, &mut window);
    assert!(window.written().iter().any(|line| line == "Checkers"), "titled");
    assert!(window.written().iter().any(|line| line == "Beginner"), "the_easiest");
    assert!(window.written().iter().any(|line| line == "Looks at 250000 positions a move. Wins score x5."), "the_hardest");
    assert!(window.written().iter().any(|line| line == "Best 2100"), "best");
}

#[test]
fn a_click_on_the_board_finds_its_square() {
    // y: covers square_at > inside
    let found = square_at(Point { x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5 });
    assert!(found == Some(Square { row: 5, column: 3 }), "the_square");
}

#[test]
fn a_click_left_of_the_board_finds_no_square() {
    // y: covers square_at > outside'1
    let found = square_at(Point { x: 5, y: 5 });
    assert!(found.is_none(), "none");
}

#[test]
fn a_click_below_the_board_finds_no_square() {
    // y: covers square_at > outside'2
    let found = square_at(Point { x: 100, y: 700 });
    assert!(found.is_none(), "none");
}

#[test]
fn a_click_on_the_first_button_finds_the_easiest_level() {
    // y: covers level_at > level > pressed
    let found = level_at(Point { x: 100, y: layout::BUTTON_TOP + 10 });
    assert!(found == Some(levels::ALL[0]), "beginner");
}

#[test]
fn a_click_on_the_second_button_finds_its_level() {
    // y: covers level_at > level'many > pressed
    let found = level_at(Point { x: 100, y: layout::BUTTON_TOP + layout::BUTTON_STEP + 10 });
    assert!(found == Some(levels::ALL[1]), "casual");
}

#[test]
fn a_click_beside_the_buttons_finds_no_level() {
    // y: covers level_at > level'many > missed'1
    let found = level_at(Point { x: 595, y: layout::BUTTON_TOP + 10 });
    assert!(found.is_none(), "none");
}

#[test]
fn a_click_above_the_buttons_finds_no_level() {
    // y: covers level_at > level'many > missed'2
    let found = level_at(Point { x: 100, y: 10 });
    assert!(found.is_none(), "none");
}

#[test]
fn a_square_has_its_corner_inside_the_gap() {
    // y: covers corner_of
    let corner = corner_of(Square { row: 1, column: 2 });
    assert!(corner == Point { x: 156, y: 84 }, "placed");
}

#[test]
fn a_square_has_its_centre_half_a_side_in() {
    // y: covers centre_of
    let centre = centre_of(Square { row: 1, column: 2 });
    assert!(centre == Point { x: 192, y: 120 }, "placed");
}

#[test]
fn a_light_square_is_light() {
    // y: covers shade_of > looks.light
    let game = new_game(levels::ALL[0]);
    let shade = shade_of(&game, Square { row: 0, column: 0 }, None);
    assert!(shade == palette::LIGHT, "light");
}

#[test]
fn the_selected_square_is_marked() {
    // y: covers shade_of > looks.selected
    let game = new_game(levels::ALL[0]);
    let shade = shade_of(&game, Square { row: 5, column: 2 }, Some(Square { row: 5, column: 2 }));
    assert!(shade == palette::SELECTED, "selected");
}

#[test]
fn the_last_move_is_marked() {
    // y: covers shade_of > looks.last
    let mut game = arranged(&[man(Side::Red, 4, 1)], Side::White);
    game.last = Some(Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 0 }, path: vec![Square { row: 4, column: 1 }], captures: Vec::new() });
    let shade = shade_of(&game, Square { row: 5, column: 0 }, None);
    assert!(shade == palette::LAST, "last");
}

#[test]
fn a_dark_square_is_dark() {
    // y: covers shade_of > looks.dark
    let game = new_game(levels::ALL[0]);
    let shade = shade_of(&game, Square { row: 5, column: 0 }, None);
    assert!(shade == palette::DARK, "dark");
}

#[test]
fn before_any_move_no_square_is_part_of_the_last() {
    // y: covers in_last_move > made.none
    let game = new_game(levels::ALL[0]);
    let part = in_last_move(&game, Square { row: 5, column: 0 });
    assert!(!part, "not_part");
}

#[test]
fn the_square_landed_on_is_part_of_the_last_move() {
    // y: covers in_last_move > made.some
    let mut game = arranged(&[man(Side::Red, 4, 1)], Side::White);
    game.last = Some(Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 0 }, path: vec![Square { row: 4, column: 1 }], captures: Vec::new() });
    let part = in_last_move(&game, Square { row: 4, column: 1 });
    assert!(part, "landed_on");
}

#[test]
fn a_player_with_no_moves_has_nowhere_to_go() {
    // y: covers landings > no_moves
    let game = arranged(&[], Side::Red);
    let found = landings(&game, Some(Square { row: 5, column: 0 }));
    assert!(found.is_empty(), "nowhere");
}

#[test]
fn the_only_move_shows_where_it_goes() {
    // y: covers landings > candidate > from_selected
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let found = landings(&game, Some(Square { row: 5, column: 0 }));
    assert!(found == vec![Square { row: 4, column: 1 }], "one_square");
}

#[test]
fn another_piece_shows_nowhere_to_go() {
    // y: covers landings > candidate > from_elsewhere
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let found = landings(&game, Some(Square { row: 0, column: 7 }));
    assert!(found.is_empty(), "nowhere");
}

#[test]
fn the_selected_piece_shows_where_it_may_go() {
    // y: covers landings > candidate'many > from_selected
    let game = new_game(levels::ALL[0]);
    let found = landings(&game, Some(Square { row: 5, column: 2 }));
    assert!(found == vec![Square { row: 4, column: 1 }, Square { row: 4, column: 3 }], "two_squares");
}

#[test]
fn with_none_selected_nothing_is_shown() {
    // y: covers landings > candidate'many > from_elsewhere
    let game = new_game(levels::ALL[0]);
    let found = landings(&game, None);
    assert!(found.is_empty(), "nowhere");
}

#[test]
fn the_board_is_drawn_in_squares() {
    // y: covers draw_board > row'many > column'many
    let game = new_game(levels::ALL[0]);
    let mut window = Window::fake(&[]);
    draw_board(&game, None, &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.fills() == 64, "every_square");
}

#[test]
fn an_empty_board_has_no_pieces_to_draw() {
    // y: covers draw_pieces > no_pieces
    let game = arranged(&[], Side::Red);
    let mut window = Window::fake(&[]);
    draw_pieces(&game, &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 0, "no_discs");
}

#[test]
fn a_lone_piece_is_drawn() {
    // y: covers draw_pieces > placed
    let game = arranged(&[man(Side::Red, 5, 0)], Side::Red);
    let mut window = Window::fake(&[]);
    draw_pieces(&game, &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 3, "three_discs");
}

#[test]
fn every_piece_is_drawn() {
    // y: covers draw_pieces > placed'many
    let game = new_game(levels::ALL[0]);
    let mut window = Window::fake(&[]);
    draw_pieces(&game, &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 72, "three_discs_each");
}

#[test]
fn with_none_selected_no_square_is_marked() {
    // y: covers draw_marks > no_marks
    let game = new_game(levels::ALL[0]);
    let mut window = Window::fake(&[]);
    draw_marks(&game, None, &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 0, "no_marks");
}

#[test]
fn the_one_square_of_the_only_move_is_marked() {
    // y: covers draw_marks > landing
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let mut window = Window::fake(&[]);
    draw_marks(&game, Some(Square { row: 5, column: 0 }), &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 1, "one_mark");
}

#[test]
fn each_square_the_selected_piece_may_go_to_is_marked() {
    // y: covers draw_marks > landing'many
    let game = new_game(levels::ALL[0]);
    let mut window = Window::fake(&[]);
    draw_marks(&game, Some(Square { row: 5, column: 2 }), &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 2, "two_marks");
}

#[test]
fn a_piece_is_drawn_as_discs() {
    // y: covers draw_piece
    let mut window = Window::fake(&[]);
    draw_piece(man(Side::Red, 5, 0), &mut window);
    assert!(window.written().is_empty(), "nothing_written");
    assert!(window.circles() == 3, "three_discs");
}

#[test]
fn a_red_king_is_red_with_a_crown() {
    // y: covers colours_of > painted.red_king
    let colours = colours_of(Piece { owner: Side::Red, king: true });
    assert!(colours == vec![palette::RED, palette::RED_RING, palette::CROWN], "crowned_red");
}

#[test]
fn a_red_man_is_red() {
    // y: covers colours_of > painted.red_man
    let colours = colours_of(Piece { owner: Side::Red, king: false });
    assert!(colours == vec![palette::RED, palette::RED_RING, palette::RED], "plain_red");
}

#[test]
fn a_white_king_is_white_with_a_crown() {
    // y: covers colours_of > painted.white_king
    let colours = colours_of(Piece { owner: Side::White, king: true });
    assert!(colours == vec![palette::WHITE, palette::WHITE_RING, palette::CROWN], "crowned_white");
}

#[test]
fn a_white_man_is_white() {
    // y: covers colours_of > painted.white_man
    let colours = colours_of(Piece { owner: Side::White, king: false });
    assert!(colours == vec![palette::WHITE, palette::WHITE_RING, palette::WHITE], "plain_white");
}

#[test]
fn escape_at_the_levels_ends_play() {
    // y: covers play > open > picked.none
    let mut window = Window::fake(&[&[Key::Escape]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "Checkers"), "levels_shown");
}

#[test]
fn a_level_is_chosen_and_a_move_made() {
    // y: covers play > open > picked.level
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
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "The computer is thinking, at 25 positions a move..."), "thinking");
    assert!(window.written().iter().any(|line| line == "Your move   Beginner   Moves 1   Best 0"), "replied");
}

#[test]
fn a_window_closed_as_a_game_starts_ends_play() {
    // y: covers play > open'many > picked.none
    let mut window = Window::fake_frames(vec![Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + 10 }] }]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
}

#[test]
fn a_game_is_played_at_the_level_chosen() {
    // y: covers play > open'many > picked.level
    let mut window = Window::fake_frames(vec![Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + 10 }] }, Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "Your move   Beginner   Moves 0   Best 0"), "at_the_level");
}

#[test]
fn a_window_closed_at_once_chooses_no_level() {
    // y: covers choose_level > closed_at_once'3
    let mut window = Window::fake(&[]);
    let forever = Forever::fake();
    let found = y_window::block_on(choose_level(0, &mut window, &forever));
    assert!(found.is_none(), "none");
}

#[test]
fn a_click_on_a_button_chooses_its_level() {
    // y: covers choose_level > choosing
    let mut window = Window::fake_frames(vec![Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + 10 }] }]);
    let forever = Forever::fake();
    let found = y_window::block_on(choose_level(0, &mut window, &forever));
    assert!(found == Some(levels::ALL[0]), "beginner");
    assert!(window.written().iter().any(|line| line == "Beginner"), "levels_shown");
}

#[test]
fn the_levels_are_shown_until_one_is_clicked() {
    // y: covers choose_level > choosing'many
    let mut window = Window::fake_frames(vec![Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: 10 }] }, Frame { keys: Vec::new(), clicks: vec![Point { x: 100, y: layout::BUTTON_TOP + layout::BUTTON_STEP + 10 }] }]);
    let forever = Forever::fake();
    let found = y_window::block_on(choose_level(0, &mut window, &forever));
    assert!(found == Some(levels::ALL[1]), "casual");
    assert!(window.frames_shown() == 2, "two_frames");
}

#[test]
fn a_window_closed_at_once_still_scores_the_game() {
    // y: covers play_game > closed_at_once'3
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let mut window = Window::fake(&[]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(won, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 2100, quitting: false }, "scored");
}

#[test]
fn space_after_a_game_chooses_another() {
    // y: covers play_game > playing > moment.over
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let mut window = Window::fake(&[&[Key::Space]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(won, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 2100, quitting: false }, "scored");
}

#[test]
fn the_computer_moves_in_a_frame() {
    // y: covers play_game > playing > moment.computers_turn
    let game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    let mut window = Window::fake_frames(vec![Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(game, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: false }, "not_quitting");
    assert!(window.written().iter().any(|line| line == "Your move   Club   Moves 1   Best 0"), "your_move");
}

#[test]
fn the_player_has_a_frame_to_act_in() {
    // y: covers play_game > playing > moment.players_turn
    let game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::Red);
    let mut window = Window::fake_frames(vec![Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(game, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: false }, "not_quitting");
    assert!(window.written().iter().any(|line| line == "Your move   Club   Moves 0   Best 0"), "your_move");
}

#[test]
fn escape_ends_a_game() {
    // y: covers play_game > playing > moment.players_turn
    let game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::Red);
    let mut window = Window::fake(&[&[Key::Escape]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(game, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: true }, "quitting");
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

#[test]
fn the_game_goes_on_after_the_computer_moves() {
    // y: covers play_game > playing'many > moment.computers_turn
    let game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::White);
    let mut window = Window::fake_frames(vec![Frame::default(), Frame::default()]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(game, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: false }, "not_quitting");
    assert!(window.frames_shown() == 2, "two_frames");
}

#[test]
fn the_player_selects_and_moves_over_two_frames() {
    // y: covers play_game > playing'many > moment.players_turn
    let game = arranged(&[man(Side::White, 2, 7), man(Side::Red, 7, 0)], Side::Red);
    let mut window = Window::fake_frames(vec![clicking(7, 0), clicking(6, 1)]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(game, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 0, quitting: false }, "not_quitting");
    assert!(window.written().iter().any(|line| line == "The computer is thinking, at 2500 positions a move..."), "thinking");
}

#[test]
fn a_click_where_the_selected_piece_may_go_moves_it() {
    // y: covers clicked > judged.moving
    let mut game = new_game(levels::ALL[0]);
    let mut selected = Some(Square { row: 5, column: 2 });
    clicked(&mut game, &mut selected, centre_of(Square { row: 4, column: 3 }));
    assert!(piece_at(&game, Square { row: 4, column: 3 }) == Some(Piece { owner: Side::Red, king: false }), "moved");
    assert!(selected.is_none(), "nothing_selected");
    assert!(game.to_move == Side::White, "white_to_move");
}

#[test]
fn a_click_on_a_piece_that_may_move_selects_it() {
    // y: covers clicked > judged.choosing
    let mut game = new_game(levels::ALL[0]);
    let mut selected = None;
    clicked(&mut game, &mut selected, centre_of(Square { row: 5, column: 2 }));
    assert!(selected == Some(Square { row: 5, column: 2 }), "selected_it");
    assert!(game.to_move == Side::Red, "red_still_to_move");
}

#[test]
fn a_click_elsewhere_selects_nothing() {
    // y: covers clicked > judged.elsewhere
    let mut game = new_game(levels::ALL[0]);
    let mut selected = Some(Square { row: 5, column: 2 });
    clicked(&mut game, &mut selected, centre_of(Square { row: 0, column: 0 }));
    assert!(selected.is_none(), "nothing_selected");
    assert!(game.to_move == Side::Red, "red_still_to_move");
}

#[test]
fn a_click_on_a_button_picks_its_level() {
    // y: covers level_clicked > place > hit
    let found = level_clicked(&[Point { x: 100, y: layout::BUTTON_TOP + 10 }]);
    assert!(found == Some(levels::ALL[0]), "beginner");
}

#[test]
fn the_first_click_on_a_button_counts() {
    // y: covers level_clicked > place'many > hit
    let found = level_clicked(&[Point { x: 100, y: 10 }, Point { x: 100, y: layout::BUTTON_TOP + 10 }]);
    assert!(found == Some(levels::ALL[0]), "beginner");
}

#[test]
fn no_clicks_pick_no_level() {
    // y: covers level_clicked > no_clicks
    let found = level_clicked(&[]);
    assert!(found.is_none(), "none");
}

#[test]
fn a_click_off_the_buttons_picks_no_level() {
    // y: covers level_clicked > place > miss
    let found = level_clicked(&[Point { x: 100, y: 10 }]);
    assert!(found.is_none(), "none");
}

#[test]
fn clicks_that_all_miss_pick_no_level() {
    // y: covers level_clicked > place'many > miss
    let found = level_clicked(&[Point { x: 100, y: 10 }, Point { x: 595, y: 10 }]);
    assert!(found.is_none(), "none");
}

#[test]
fn no_clicks_change_nothing() {
    // y: covers act_on_clicks > no_clicks
    let mut game = new_game(levels::ALL[0]);
    let mut selected = Some(Square { row: 5, column: 2 });
    act_on_clicks(&mut game, &mut selected, &[]);
    assert!(selected == Some(Square { row: 5, column: 2 }), "still_selected");
    assert!(game.to_move == Side::Red, "red_still_to_move");
}

#[test]
fn one_click_selects_a_man() {
    // y: covers act_on_clicks > place
    let mut game = new_game(levels::ALL[0]);
    let mut selected = None;
    act_on_clicks(&mut game, &mut selected, &[centre_of(Square { row: 5, column: 2 })]);
    assert!(selected == Some(Square { row: 5, column: 2 }), "selected_him");
}

#[test]
fn two_clicks_in_a_frame_make_a_move() {
    // y: covers act_on_clicks > place'many
    let mut game = new_game(levels::ALL[0]);
    let mut selected = None;
    act_on_clicks(&mut game, &mut selected, &[centre_of(Square { row: 5, column: 2 }), centre_of(Square { row: 4, column: 3 })]);
    assert!(piece_at(&game, Square { row: 4, column: 3 }) == Some(Piece { owner: Side::Red, king: false }), "moved");
    assert!(game.to_move == Side::White, "white_to_move");
}

#[test]
fn the_only_move_is_found() {
    // y: covers move_to > candidate > fits
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let found = move_to(&game, Some(Square { row: 5, column: 0 }), Some(Square { row: 4, column: 1 }));
    assert!(found == Some(Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 0 }, path: vec![Square { row: 4, column: 1 }], captures: Vec::new() }), "found_it");
}

#[test]
fn a_move_among_many_is_found() {
    // y: covers move_to > candidate'many > fits
    let game = new_game(levels::ALL[0]);
    let found = move_to(&game, Some(Square { row: 5, column: 2 }), Some(Square { row: 4, column: 3 }));
    assert!(found == Some(Move { piece: Piece { owner: Side::Red, king: false }, from: Square { row: 5, column: 2 }, path: vec![Square { row: 4, column: 3 }], captures: Vec::new() }), "found_it");
}

#[test]
fn with_no_moves_none_is_found() {
    // y: covers move_to > no_moves
    let game = arranged(&[], Side::Red);
    let found = move_to(&game, Some(Square { row: 5, column: 0 }), Some(Square { row: 4, column: 1 }));
    assert!(found.is_none(), "no_move");
}

#[test]
fn the_only_move_is_not_found_from_another_square() {
    // y: covers move_to > candidate > differs'1
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let found = move_to(&game, Some(Square { row: 6, column: 1 }), Some(Square { row: 4, column: 1 }));
    assert!(found.is_none(), "no_move");
}

#[test]
fn the_only_move_is_not_found_to_another_square() {
    // y: covers move_to > candidate > differs'2
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let found = move_to(&game, Some(Square { row: 5, column: 0 }), Some(Square { row: 3, column: 2 }));
    assert!(found.is_none(), "no_move");
}

#[test]
fn no_move_is_found_from_a_blocked_man() {
    // y: covers move_to > candidate'many > differs'1
    let game = new_game(levels::ALL[0]);
    let found = move_to(&game, Some(Square { row: 6, column: 1 }), Some(Square { row: 5, column: 0 }));
    assert!(found.is_none(), "no_move");
}

#[test]
fn no_move_is_found_two_squares_away() {
    // y: covers move_to > candidate'many > differs'2
    let game = new_game(levels::ALL[0]);
    let found = move_to(&game, Some(Square { row: 5, column: 2 }), Some(Square { row: 3, column: 2 }));
    assert!(found.is_none(), "no_move");
}

#[test]
fn the_piece_of_the_only_move_may_move() {
    // y: covers may_move_from > candidate > movable
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let can = may_move_from(&game, Some(Square { row: 5, column: 0 }));
    assert!(can, "movable");
}

#[test]
fn a_front_man_may_move() {
    // y: covers may_move_from > candidate'many > movable
    let game = new_game(levels::ALL[0]);
    let can = may_move_from(&game, Some(Square { row: 5, column: 2 }));
    assert!(can, "movable");
}

#[test]
fn with_no_moves_nothing_may_move() {
    // y: covers may_move_from > no_moves
    let game = arranged(&[], Side::Red);
    let can = may_move_from(&game, Some(Square { row: 5, column: 0 }));
    assert!(!can, "not_movable");
}

#[test]
fn another_square_may_not_move() {
    // y: covers may_move_from > candidate > unmovable
    let game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 0, 7)], Side::Red);
    let can = may_move_from(&game, Some(Square { row: 6, column: 1 }));
    assert!(!can, "not_movable");
}

#[test]
fn a_blocked_man_may_not_move() {
    // y: covers may_move_from > candidate'many > unmovable
    let game = new_game(levels::ALL[0]);
    let can = may_move_from(&game, Some(Square { row: 6, column: 1 }));
    assert!(!can, "not_movable");
}
