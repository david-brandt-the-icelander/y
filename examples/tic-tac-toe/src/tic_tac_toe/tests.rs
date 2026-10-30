//! Lowered from `y/src/tic_tac_toe.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

/// A game with crosses and noughts in the given squares.
///
/// - `crosses`: the squares that hold a cross.
/// - `noughts`: the squares that hold a nought.
/// - `to_move`: whose turn it is.
/// - Returns the game.
fn marked(crosses: &[u32], noughts: &[u32], to_move: Mark) -> Game {
    let mut squares = list::repeated(None, board::SQUARES);
    for square in crosses { // y: #square, else #no_crosses
        list::set_number(&mut squares, *square, Some(Mark::Cross));
    }
    for square in noughts { // y: #square, else #no_noughts
        list::set_number(&mut squares, *square, Some(Mark::Nought));
    }
    Game { squares, to_move }
}

#[test]
fn a_move_is_read() {
    // y: covers read_move > a_number > judged.empty
    let game = new_game();
    let chosen = read_move(&game, " 5 ").expect("when");
    assert!(chosen == 5, "the_middle");
}

#[test]
fn a_word_is_not_a_move() {
    // y: covers read_move > not_a_number
    let game = new_game();
    let refused = read_move(&game, "five");
    assert!(refused == Err(MoveError::NotANumber), "refused_as_not_a_number");
}

#[test]
fn there_is_no_tenth_square() {
    // y: covers read_move > a_number > judged.off_board'2
    let game = new_game();
    let refused = read_move(&game, "10");
    assert!(refused == Err(MoveError::NoSuchSquare), "refused_as_no_square");
}

#[test]
fn a_marked_square_is_taken() {
    // y: covers read_move > a_number > judged.taken
    let game = marked(&[5], &[], Mark::Nought);
    let refused = read_move(&game, "5");
    assert!(refused == Err(MoveError::Taken), "refused_as_taken");
}

#[test]
fn the_players_take_turns() {
    // y: covers place, other > current.cross, other > current.nought
    let mut game = new_game();
    place(&mut game, 5);
    place(&mut game, 1);
    assert!(list::at_number(&game.squares, 5) == Some(Mark::Cross), "a_cross_first");
    assert!(list::at_number(&game.squares, 1) == Some(Mark::Nought), "then_a_nought");
    assert!(game.to_move == Mark::Cross, "then_a_cross_again");
}

#[test]
fn three_in_a_row_wins() {
    // y: covers over, winner > line'many > three
    let game = marked(&[3, 5, 7], &[1, 2], Mark::Nought);
    let found = winner(&game);
    assert!(found == Some(Mark::Cross), "crosses_won: Expected the diagonal, the last line looked at, to be found.");
    assert!(over(&game), "ended");
}

#[test]
fn a_full_board_with_no_line_is_a_draw() {
    // y: covers winner > line'many > not_three'2, winner > line'many > not_three'3,
    // open_squares > number'many > marked, value_of > now.drawn
    let game = marked(&[1, 2, 6, 7, 9], &[3, 4, 5, 8], Mark::Nought);
    let value = value_of(&game);
    assert!(value == 0, "drawn");
    assert!(over(&game), "ended");
}

#[test]
fn the_computer_wins_rather_than_blocks() {
    // y: covers computer_move > square'many > better, computer_move > square'many > no_better'lt,
    // winner > line > three, value_of > now.lost, value_of > now.open > square, value_of > now.open > square'many
    let game = marked(&[1, 2], &[4, 5], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 6, "completes_its_row");
}

#[test]
fn the_computer_blocks_a_line() {
    let game = marked(&[1, 2], &[5], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 3, "blocks_the_top_row");
}

#[test]
fn the_board_shows_marks_and_numbers() {
    // y: covers shown > first'many > number'many, square_shown > held.empty, square_shown > held.marked
    let game = marked(&[5], &[1], Mark::Cross);
    let picture = shown(&game);
    assert!(picture == " O | 2 | 3\n---+---+---\n 4 | X | 6\n---+---+---\n 7 | 8 | 9", "drawn");
}

#[test]
fn a_player_who_plays_perfectly_draws() {
    // y: covers computer_move > square'many > no_better'eq, play > playing'many > persons_turn,
    // play > playing'many > computers_turn
    let mut console = Console::fake(&["5", "8", "3", "4", "9"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("The board is full. A draw."), "announced");
}

#[test]
fn a_player_who_plays_carelessly_loses() {
    // y: covers announce > won
    let mut console = Console::fake(&["1", "2", "4"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("O has three in a row, and wins."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers play > playing'many > persons_turn > persons_move'fails, persons_move > line_read > attempted.invalid,
    // persons_move > closed, complaint > wrong.no_square, complaint > wrong.taken
    let mut console = Console::fake(&["five", "10", "5", "5"], false);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    let written = console.written();
    assert!(written.iter().any(|line| line == "Type the number of a square, from 1 to 9."), "told_the_form");
    assert!(written.iter().any(|line| line == "There is no such square."), "told_no_square");
    assert!(written.iter().any(|line| line == "That square is taken."), "told_taken");
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers winner > line'many > not_three'1, open_squares > number'many > empty,
    // play > playing > persons_turn > persons_move'fails, persons_move > typed'fails
    let mut console = Console::fake(&[], true);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn square_zero_is_not_on_the_board() {
    // y: covers read_move > a_number > judged.off_board'1
    let game = new_game();
    let refused = read_move(&game, "0");
    assert!(refused == Err(MoveError::NoSuchSquare), "no_such_square");
}

#[test]
fn the_computer_takes_the_last_square() {
    // y: covers computer_move > square > better
    let game = marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 9, "the_ninth");
}

#[test]
fn an_end_cannot_be_shown_without_the_board() {
    // y: covers announce > write_line'1'fails
    let game = marked(&[1, 3, 4, 8, 9], &[2, 5, 6, 7], Mark::Cross);
    let mut console = Console::fake_failing_writes(&[], 0);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_draw_cannot_be_announced() {
    // y: covers announce > drawn > write_line'2'fails
    let game = marked(&[1, 3, 4, 8, 9], &[2, 5, 6, 7], Mark::Cross);
    let mut console = Console::fake_failing_writes(&[], 1);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_win_cannot_be_announced() {
    // y: covers announce > won > write_line'3'fails
    let game = marked(&[3, 5, 7], &[1, 2], Mark::Nought);
    let mut console = Console::fake_failing_writes(&[], 1);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_move_cannot_be_asked_for_without_the_board() {
    // y: covers persons_move > write_line'1'fails
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&[], 0);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_move_cannot_be_asked_for() {
    // y: covers persons_move > write'fails
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_wrong_move_cannot_be_complained_of() {
    // y: covers persons_move > line_read > attempted.invalid > write_line'2'fails, complaint > wrong.not_a_move
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&["x"], 2);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers new_game, play > write_line'1'fails
    let mut console = Console::fake_failing_writes(&[], 0);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_full_board_is_over_at_once() {
    // y: covers play > over_at_once, announce > drawn
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8, 9], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn a_console_that_cannot_announce_at_once_ends_play() {
    // y: covers play > over_at_once > announce'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8, 9], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn the_player_fills_the_last_square() {
    // y: covers play > playing > persons_turn, persons_move > line_read > attempted.valid
    let mut console = Console::fake(&["9"], false);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn a_console_that_cannot_announce_the_players_last_move_ends_play() {
    // y: covers play > playing > persons_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["9"], 3);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn the_computer_fills_the_last_square() {
    // y: covers play > playing > computers_turn
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Nought), &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn a_console_that_cannot_tell_the_computers_last_move_ends_play() {
    // y: covers play > playing > computers_turn > write_line'2'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Nought), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_the_computers_last_move_ends_play() {
    // y: covers play > playing > computers_turn > announce'fails
    let mut console = Console::fake_failing_writes(&[], 2);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4, 8], &[2, 5, 6, 7], Mark::Nought), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_tell_the_computers_move_ends_play() {
    // y: covers play > playing'many > computers_turn > write_line'2'fails
    let mut console = Console::fake_failing_writes(&["8"], 3);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_after_both_have_moved_ends_play() {
    // y: covers play > playing'many > persons_turn > announce'fails,
    // play > playing'many > computers_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["8"], 4);
    let forever = Forever::fake();
    let ended = play(marked(&[1, 3, 4], &[2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}
