//! Lowered from `y/src/tic_tac_toe.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;
use y_collections::list;

/// A game with crosses and noughts in the given squares.
///
/// - `crosses`: the squares that hold a cross.
/// - `noughts`: the squares that hold a nought.
/// - `to_move`: whose turn it is.
/// - Returns the game.
fn marked(crosses: &List<u32>, noughts: &List<u32>, to_move: Mark) -> Game {
    let mut squares = List::from(list::repeated(None, board::SQUARES));
    for square in crosses { // y: #square, else #no_crosses
        list::set_number(&mut squares, *square, Some(Mark::Cross));
    }
    for square in noughts { // y: #square, else #no_noughts
        list::set_number(&mut squares, *square, Some(Mark::Nought));
    }
    Game { squares, to_move }
}

#[test]
fn a_game_starts_with_an_empty_board_and_crosses_to_move() {
    // y: covers new_game
    let started = new_game();
    assert!(started.squares == List::from(list::repeated(None, board::SQUARES)), "empty");
    assert!(started.to_move == Mark::Cross, "crosses_to_move");
}

#[test]
fn a_move_is_read() {
    // y: covers read_move > a_number > judged.empty
    let game = new_game();
    let chosen = read_move(&game, &YString::new(" 5 ")).expect("when");
    assert!(chosen == 5, "the_middle");
}

#[test]
fn a_word_is_not_a_move() {
    // y: covers read_move > not_a_number
    let game = new_game();
    let refused = read_move(&game, &YString::new("five"));
    assert!(refused == Err(MoveError::NotANumber), "refused_as_not_a_number");
}

#[test]
fn square_zero_is_not_on_the_board() {
    // y: covers read_move > a_number > judged.off_board'1
    let game = new_game();
    let refused = read_move(&game, &YString::new("0"));
    assert!(refused == Err(MoveError::NoSuchSquare), "no_such_square");
}

#[test]
fn there_is_no_tenth_square() {
    // y: covers read_move > a_number > judged.off_board'2
    let game = new_game();
    let refused = read_move(&game, &YString::new("10"));
    assert!(refused == Err(MoveError::NoSuchSquare), "refused_as_no_square");
}

#[test]
fn a_marked_square_is_taken() {
    // y: covers read_move > a_number > judged.taken
    let game = marked(&list![5], &List::new(), Mark::Nought);
    let refused = read_move(&game, &YString::new("5"));
    assert!(refused == Err(MoveError::Taken), "refused_as_taken");
}

#[test]
fn a_move_puts_a_mark_and_passes_the_turn() {
    // y: covers place
    let mut game = new_game();
    place(&mut game, 5);
    assert!(list::at_number(&game.squares, 5) == Some(Mark::Cross), "a_cross_in_the_middle");
    assert!(game.to_move == Mark::Nought, "noughts_to_move");
}

#[test]
fn the_computer_takes_the_last_square() {
    // y: covers computer_move > square > better
    let game = marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 9, "the_ninth");
}

#[test]
fn the_computer_wins_rather_than_blocks() {
    // y: covers computer_move > square'many > better
    let game = marked(&list![1, 2], &list![4, 5], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 6, "completes_its_row");
}

#[test]
fn the_computer_blocks_a_line() {
    // y: covers computer_move > square'many > no_better'lt
    let game = marked(&list![1, 2], &list![5], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 3, "blocks_the_top_row");
}

#[test]
fn of_squares_as_good_the_computer_takes_the_first() {
    // y: covers computer_move > square'many > no_better'eq
    let game = marked(&list![1, 9], &list![5], Mark::Nought);
    let chosen = computer_move(&game);
    assert!(chosen == 2, "the_first_of_the_edges");
}

#[test]
fn a_line_of_three_ends_the_game() {
    // y: covers over
    let game = marked(&list![3, 5, 7], &list![1, 2], Mark::Nought);
    let ended = over(&game);
    assert!(ended, "won");
}

#[test]
fn a_full_board_ends_the_game() {
    // y: covers over
    let game = marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross);
    let ended = over(&game);
    assert!(ended, "full");
}

#[test]
fn a_new_game_is_not_over() {
    // y: covers over
    let game = new_game();
    let ended = over(&game);
    assert!(!ended, "still_playing");
}

#[test]
fn a_line_along_the_top_row_is_found_first() {
    // y: covers winner > line > three
    let game = marked(&list![1, 2, 3], &list![4, 5], Mark::Nought);
    let found = winner(&game);
    assert!(found == Some(Mark::Cross), "crosses_won");
}

#[test]
fn three_in_a_row_wins() {
    // y: covers winner > line'many > three
    let game = marked(&list![3, 5, 7], &list![1, 2], Mark::Nought);
    let found = winner(&game);
    assert!(found == Some(Mark::Cross), "crosses_won: Expected the diagonal, the last line looked at, to be found.");
}

#[test]
fn an_empty_board_has_no_winner() {
    // y: covers winner > line'many > not_three'1
    let game = new_game();
    let found = winner(&game);
    assert!(found.is_none(), "no_winner");
}

#[test]
fn a_line_with_two_marks_that_differ_is_not_three() {
    // y: covers winner > line'many > not_three'2
    let game = marked(&list![1], &list![2], Mark::Cross);
    let found = winner(&game);
    assert!(found.is_none(), "no_winner");
}

#[test]
fn two_in_a_row_are_not_three() {
    // y: covers winner > line'many > not_three'3
    let game = marked(&list![1, 2], &list![4], Mark::Nought);
    let found = winner(&game);
    assert!(found.is_none(), "no_winner");
}

#[test]
fn crosses_are_followed_by_noughts() {
    // y: covers other > current.cross
    let next = other(Mark::Cross);
    assert!(next == Mark::Nought, "noughts_next");
}

#[test]
fn noughts_are_followed_by_crosses() {
    // y: covers other > current.nought
    let next = other(Mark::Nought);
    assert!(next == Mark::Cross, "crosses_next");
}

#[test]
fn every_square_of_a_new_game_is_open() {
    // y: covers open_squares > number'many > empty
    let game = new_game();
    let open = open_squares(&game);
    assert!(open == list![1, 2, 3, 4, 5, 6, 7, 8, 9], "all_nine");
}

#[test]
fn a_full_board_has_no_open_square() {
    // y: covers open_squares > number'many > marked
    let game = marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross);
    let open = open_squares(&game);
    assert!(list::count(&open) == 0, "none_open");
}

#[test]
fn a_lost_game_costs_more_the_more_squares_are_left() {
    // y: covers value_of > now.lost
    let game = marked(&list![3, 5, 7], &list![1, 2], Mark::Nought);
    let value = value_of(&game);
    assert!(value == -5, "lost_with_four_left");
}

#[test]
fn a_full_board_with_no_line_is_a_draw() {
    // y: covers value_of > now.drawn
    let game = marked(&list![1, 2, 6, 7, 9], &list![3, 4, 5, 8], Mark::Nought);
    let value = value_of(&game);
    assert!(value == 0, "drawn");
}

#[test]
fn a_last_square_that_makes_no_line_is_worth_nothing() {
    // y: covers value_of > now.open > square
    let game = marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Cross);
    let value = value_of(&game);
    assert!(value == 0, "drawn");
}

#[test]
fn a_line_that_can_be_made_at_once_is_worth_most() {
    // y: covers value_of > now.open > square'many
    let game = marked(&list![1, 2], &list![4, 5], Mark::Cross);
    let value = value_of(&game);
    assert!(value == 5, "wins_at_once");
}

#[test]
fn the_board_shows_marks_and_numbers() {
    // y: covers shown > first'many > number'many
    let game = marked(&list![5], &list![1], Mark::Cross);
    let picture = shown(&game);
    assert!(picture == " O | 2 | 3\n---+---+---\n 4 | X | 6\n---+---+---\n 7 | 8 | 9", "drawn");
}

#[test]
fn an_empty_square_shows_its_number() {
    // y: covers square_shown > held.empty
    let game = new_game();
    let drawn = square_shown(&game, 5);
    assert!(drawn == "5", "its_number");
}

#[test]
fn a_marked_square_shows_its_mark() {
    // y: covers square_shown > held.marked
    let game = marked(&list![5], &list![1], Mark::Cross);
    let drawn = square_shown(&game, 1);
    assert!(drawn == "O", "its_mark");
}

#[test]
fn a_full_board_is_over_at_once() {
    // y: covers play > over_at_once
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    play(marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn the_player_fills_the_last_square() {
    // y: covers play > playing > persons_turn
    let mut console = Console::fake(&["9"], false);
    let forever = Forever::fake();
    play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn the_computer_fills_the_last_square() {
    // y: covers play > playing > computers_turn
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Nought), &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn a_player_who_plays_perfectly_draws() {
    // y: covers play > playing'many > persons_turn
    let mut console = Console::fake(&["5", "8", "3", "4", "9"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "announced");
}

#[test]
fn a_player_who_plays_carelessly_loses() {
    // y: covers play > playing'many > computers_turn
    let mut console = Console::fake(&["1", "2", "4"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("O has three in a row, and wins."), "announced");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers play > write_line'1'fails
    let mut console = Console::fake_failing_writes(&[], 0);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers play > playing > persons_turn > persons_move'fails
    let mut console = Console::fake(&[], true);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn input_that_closes_after_a_wrong_move_ends_play() {
    // y: covers play > playing'many > persons_turn > persons_move'fails
    let mut console = Console::fake(&["0"], false);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_console_that_cannot_tell_the_computers_last_move_ends_play() {
    // y: covers play > playing > computers_turn > write_line'2'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Nought), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_tell_the_computers_move_ends_play() {
    // y: covers play > playing'many > computers_turn > write_line'2'fails
    let mut console = Console::fake_failing_writes(&["8"], 3);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_at_once_ends_play() {
    // y: covers play > over_at_once > announce'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_the_players_last_move_ends_play() {
    // y: covers play > playing > persons_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["9"], 3);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_the_computers_last_move_ends_play() {
    // y: covers play > playing > computers_turn > announce'fails
    let mut console = Console::fake_failing_writes(&[], 2);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Nought), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_after_a_retyped_last_move_ends_play() {
    // y: covers play > playing'many > persons_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["0", "9"], 6);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4, 8], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_after_both_have_moved_ends_play() {
    // y: covers play > playing'many > computers_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["8"], 4);
    let forever = Forever::fake();
    let ended = play(marked(&list![1, 3, 4], &list![2, 5, 6, 7], Mark::Cross), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_players_move_is_made() {
    // y: covers persons_move > line_read > attempted.valid
    let mut game = new_game();
    let mut console = Console::fake(&["5"], false);
    let forever = Forever::fake();
    persons_move(&mut game, &mut console, &forever).expect("when");
    assert!(list::at_number(&game.squares, 5) == Some(Mark::Cross), "a_cross_in_the_middle");
    assert!(game.to_move == Mark::Nought, "noughts_to_move");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers persons_move > line_read > attempted.invalid
    let mut game = new_game();
    let mut console = Console::fake(&["five"], false);
    let forever = Forever::fake();
    persons_move(&mut game, &mut console, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("Type the number of a square, from 1 to 9."), "told_the_form");
    assert!(game == new_game(), "unchanged");
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
fn a_move_cannot_be_read_from_a_failing_console() {
    // y: covers persons_move > typed'fails
    let mut game = new_game();
    let mut console = Console::fake(&[], true);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_closed_console_gives_no_move() {
    // y: covers persons_move > closed
    let mut game = new_game();
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_wrong_move_cannot_be_complained_of() {
    // y: covers persons_move > line_read > attempted.invalid > write_line'2'fails
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&["x"], 2);
    let forever = Forever::fake();
    let moved = persons_move(&mut game, &mut console, &forever);
    assert!(moved == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_player_who_types_no_number_is_asked_for_one() {
    // y: covers complaint > wrong.not_a_move
    let said = complaint(MoveError::NotANumber);
    assert!(said == "Type the number of a square, from 1 to 9.", "asks_for_a_number");
}

#[test]
fn a_player_who_names_no_square_is_told_so() {
    // y: covers complaint > wrong.no_square
    let said = complaint(MoveError::NoSuchSquare);
    assert!(said == "There is no such square.", "says_there_is_none");
}

#[test]
fn a_player_who_names_a_taken_square_is_told_so() {
    // y: covers complaint > wrong.taken
    let said = complaint(MoveError::Taken);
    assert!(said == "That square is taken.", "says_it_is_taken");
}

#[test]
fn a_draw_is_announced() {
    // y: covers announce > drawn
    let game = marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross);
    let mut console = Console::fake(&[], false);
    announce(&game, &mut console).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("The board is full. A draw."), "drawn");
}

#[test]
fn a_win_is_announced() {
    // y: covers announce > won
    let game = marked(&list![3, 5, 7], &list![1, 2], Mark::Nought);
    let mut console = Console::fake(&[], false);
    announce(&game, &mut console).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("X has three in a row, and wins."), "crosses_won");
}

#[test]
fn an_end_cannot_be_shown_without_the_board() {
    // y: covers announce > write_line'1'fails
    let game = marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross);
    let mut console = Console::fake_failing_writes(&[], 0);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_draw_cannot_be_announced() {
    // y: covers announce > drawn > write_line'2'fails
    let game = marked(&list![1, 3, 4, 8, 9], &list![2, 5, 6, 7], Mark::Cross);
    let mut console = Console::fake_failing_writes(&[], 1);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_win_cannot_be_announced() {
    // y: covers announce > won > write_line'3'fails
    let game = marked(&list![3, 5, 7], &list![1, 2], Mark::Nought);
    let mut console = Console::fake_failing_writes(&[], 1);
    let said = announce(&game, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}
