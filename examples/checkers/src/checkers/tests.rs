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
    for placed in pieces {
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
    // y: covers starting_piece#placed.light, starting_piece#placed.white_men, starting_piece#placed.red_men,
    // starting_piece#placed.between, placed_pieces#held.empty, placed_pieces#held.piece, standing#judged.playing
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
    // y: covers legal_moves#no_jump, all_steps#open, all_steps#blocked, own_pieces#mine, own_pieces#theirs,
    // directions_of#moving.red, directions_of#moving.white, chains#cannot_jump, jumpable#inside, jumpable#outside,
    // belongs_to#held.empty, belongs_to#held.piece, apply#quiet, other#current.red
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
    // y: covers legal_moves#must_jump, chains#can_jump, chains#ends_here, chains_after#uncrowned, apply#capturing
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
    // y: covers chains#goes_on
    let game = arranged(&[man(Side::Red, 6, 1), man(Side::White, 5, 2), man(Side::White, 3, 4)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.len() == 1, "one_move");
    assert!(moves[0].path == vec![Square { row: 4, column: 3 }, Square { row: 2, column: 5 }], "two_landings");
    assert!(moves[0].captures == vec![Square { row: 5, column: 2 }, Square { row: 3, column: 4 }], "two_captures");
}

#[test]
fn a_man_crowned_by_a_jump_stops() {
    // y: covers chains_after#crowned, far_row#heading_up
    let mut game = arranged(&[man(Side::Red, 2, 1), man(Side::White, 1, 2), man(Side::White, 1, 4)], Side::Red);
    let moves = legal_moves(&game);
    apply(&mut game, moves[0].clone()); // y: a copy
    assert!(moves[0].path == vec![Square { row: 0, column: 3 }], "one_jump");
    assert!(piece_at(&game, Square { row: 0, column: 3 }) == Some(Piece { owner: Side::Red, king: true }), "crowned");
}

#[test]
fn a_king_moves_both_ways() {
    // y: covers directions_of#moving.king
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 0, 1)], Side::Red);
    let moves = legal_moves(&game);
    assert!(moves.len() == 4, "four_ways");
}

#[test]
fn white_is_crowned_on_the_bottom_row() {
    // y: covers far_row#heading_down, other#current.white
    let mut game = arranged(&[man(Side::White, 6, 1), man(Side::Red, 0, 7)], Side::White);
    let moves = legal_moves(&game);
    apply(&mut game, moves[0].clone()); // y: a copy
    assert!(piece_at(&game, list::last(&moves[0].path)) == Some(Piece { owner: Side::White, king: true }), "crowned");
    assert!(game.to_move == Side::Red, "red_to_move");
}

#[test]
fn a_win_scores_by_level_and_moves() {
    // y: covers standing#judged.won, score#won
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
    // y: covers standing#judged.lost, score#unfinished
    let game = arranged(&[man(Side::White, 3, 2)], Side::Red);
    let reached = standing(&game);
    let points = score(&game);
    assert!(reached == Outcome::Lost, "lost");
    assert!(points == 0, "nothing");
}

#[test]
fn quiet_turns_draw_the_game() {
    // y: covers standing#judged.drawn
    let mut game = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    game.quiet = game_rules::QUIET_LIMIT;
    let reached = standing(&game);
    assert!(reached == Outcome::Drawn, "drawn");
}

#[test]
fn pieces_are_worth_their_rank() {
    // y: covers value_for#judged.playing, material#counted.other, material#counted.king, material#counted.man
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::Red, 6, 1), man(Side::White, 1, 2)], Side::Red);
    let for_red = value_for(&game, Side::Red);
    let for_white = value_for(&game, Side::White);
    assert!(for_red == 5, "red_ahead: Expected 5 for the king and 3 for the man, less 3 for white's man.");
    assert!(for_white == -5, "white_behind");
}

#[test]
fn an_ended_game_is_worth_its_end() {
    // y: covers value_for#judged.losing, value_for#judged.winning, value_for#judged.drawn
    let stuck = arranged(&[man(Side::Red, 4, 3)], Side::White);
    let mut drawn = arranged(&[man(Side::Red, 5, 0), man(Side::White, 2, 7)], Side::Red);
    drawn.quiet = game_rules::QUIET_LIMIT;
    let for_white = value_for(&stuck, Side::White);
    let for_red = value_for(&stuck, Side::Red);
    let for_either = value_for(&drawn, Side::Red);
    assert!(for_white == -search::WIN_VALUE, "white_lost");
    assert!(for_red == search::WIN_VALUE, "red_won");
    assert!(for_either == 0, "nothing_for_a_draw");
}

#[test]
fn the_computer_keeps_its_man_out_of_reach() {
    // y: covers computer_move#deepening, computer_move#finished, computer_move#ran_out, computer_move#deep_enough,
    // searched#judged.leaf, searched#judged.spent, searched#judged.inner, searched#owners_turn,
    // searched#others_turn, best_sampled#top, best_sampled#lower, best_sampled#settled
    let mut random = Random::seeded(7);
    let mut game = arranged(&[man(Side::White, 2, 3), man(Side::Red, 4, 5)], Side::White);
    game.level = levels::ALL[1];
    let chosen = computer_move(&game, &mut random);
    assert!(chosen.path == vec![Square { row: 3, column: 2 }], "out_of_reach");
}

#[test]
fn the_computer_plays_out_games_to_choose_an_opening() {
    // y: covers best_sampled#contested, sampled_value#sampling, sampled_value#sampled, sampled_value#any, playout#going,
    // playout#ended
    let mut random = Random::seeded(7);
    let mut game = new_game(levels::ALL[1]);
    let opening = legal_moves(&game)[0].clone(); // y: a copy
    apply(&mut game, opening);
    let chosen = computer_move(&game, &mut random);
    assert!(legal_moves(&game).contains(&chosen), "a_white_move");
}

#[test]
fn without_checks_the_pieces_decide() {
    // y: covers sampled_value#none
    let mut random = Random::seeded(7);
    let game = arranged(&[king(Side::Red, 4, 3), man(Side::White, 1, 2)], Side::Red);
    let worth = sampled_value(&game, Side::Red, 0, &mut random);
    assert!(worth == 2, "the_pieces");
}

#[test]
fn a_click_finds_its_square() {
    // y: covers square_at#inside, square_at#outside
    let on = square_at(Point { x: 12 + 72 * 3 + 5, y: 12 + 72 * 5 + 5 });
    let off = square_at(Point { x: 5, y: 5 });
    assert!(on == Some(Square { row: 5, column: 3 }), "found");
    assert!(off.is_none(), "none");
}

#[test]
fn a_click_finds_its_level() {
    // y: covers level_at#pressed, level_at#missed
    let on = level_at(Point { x: 100, y: layout::BUTTON_TOP + layout::BUTTON_STEP + 10 });
    let off = level_at(Point { x: 100, y: 10 });
    assert!(on == Some(levels::ALL[1]), "casual");
    assert!(off.is_none(), "none");
}

#[test]
fn squares_are_shaded() {
    // y: covers shade_of#looks.light, shade_of#looks.selected, shade_of#looks.last, shade_of#looks.dark,
    // in_last_move#made.none, in_last_move#made.some
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
    // y: covers landings#from_selected, landings#from_elsewhere
    let game = new_game(levels::ALL[0]);
    let some = landings(&game, Some(Square { row: 5, column: 2 }));
    let none = landings(&game, None);
    assert!(some == vec![Square { row: 4, column: 1 }, Square { row: 4, column: 3 }], "two_squares");
    assert!(none.is_empty(), "nothing");
}

#[test]
fn pieces_are_painted_by_player_and_rank() {
    // y: covers colours_of#painted.red_king, colours_of#painted.red_man, colours_of#painted.white_king,
    // colours_of#painted.white_man
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
    // y: covers banner#shown.your_move, banner#shown.thinking, banner#shown.won, banner#shown.lost, banner#shown.drawn
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
    // y: covers clicked#judged.elsewhere
    let mut game = new_game(levels::ALL[0]);
    let mut selected = Some(Square { row: 5, column: 2 });
    clicked(&mut game, &mut selected, centre_of(Square { row: 0, column: 0 }));
    assert!(selected.is_none(), "nothing_selected");
    assert!(game.to_move == Side::Red, "red_still_to_move");
}

#[test]
fn escape_at_the_levels_ends_play() {
    // y: covers play#open, play#picked.none, play#closed, choose_level#choosing, choose_level#chosen_or_stopped
    let mut window = Window::fake(&[&[Key::Escape]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = y_window::block_on(play(&mut window, &mut random, &forever));
    assert!(best == 0, "no_score");
    assert!(window.written().iter().any(|line| line == "Checkers"), "levels_shown");
}

#[test]
fn a_level_is_chosen_and_a_move_made() {
    // y: covers choose_level#miss, choose_level#hit, play#picked.level, play_game#playing, play_game#stopped,
    // play_game#moment.players_turn, play_game#moment.computers_turn, clicked#judged.choosing,
    // clicked#judged.moving, move_to#fits, move_to#differs, may_move_from#movable, may_move_from#unmovable
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
    // y: covers play_game#moment.over
    let mut won = arranged(&[man(Side::Red, 4, 3)], Side::White);
    won.turns = 59;
    let mut window = Window::fake(&[&[Key::Space]]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = y_window::block_on(play_game(won, 0, &mut window, &mut random, &forever));
    assert!(ended == Ending { points: 2100, quitting: false }, "scored");
}
