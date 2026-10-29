//! Lowered from `y/src/nim.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

#[test]
fn a_move_is_read() {
    // y: covers parse_move#two, parse_move#numbers, check#heap_exists, check#judged.fits
    let game = new_game();
    let chosen = read_move(&game, " 2  3 ").expect("when");
    assert!(chosen.heap == 2, "heap_two");
    assert!(chosen.take == 3, "three_counters");
}

#[test]
fn one_number_is_not_a_move() {
    // y: covers parse_move#not_two, parse_move!move_error.not_two_numbers, read_move!move_error.not_two_numbers
    let game = new_game();
    let refused = read_move(&game, "3");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn two_words_are_not_a_move() {
    // y: covers parse_move#not_numbers
    let game = new_game();
    let refused = read_move(&game, "two three");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn there_is_no_fourth_heap() {
    // y: covers check#no_such_heap, check!move_error.no_such_heap, read_move!move_error.no_such_heap
    let game = new_game();
    let refused = read_move(&game, "4 1");
    assert!(refused == Err(MoveError::NoSuchHeap), "refused_as_no_heap");
}

#[test]
fn taking_nothing_is_not_a_move() {
    // y: covers check#judged.nothing_taken, check!move_error.nothing_taken, read_move!move_error.nothing_taken
    let game = new_game();
    let refused = read_move(&game, "1 0");
    assert!(refused == Err(MoveError::NothingTaken), "refused_as_nothing_taken");
}

#[test]
fn taking_too_many_is_not_a_move() {
    // y: covers check#judged.too_many, check!move_error.too_many, read_move!move_error.too_many
    let game = new_game();
    let refused = read_move(&game, "1 4");
    assert!(refused == Err(MoveError::TooMany), "refused_as_too_many");
}

#[test]
fn the_computer_finds_the_winning_move() {
    // y: covers computer_move#searching, computer_move#not_this_heap, computer_move#winning
    let game = Game { heaps: vec![1, 2, 4] };
    let chosen = computer_move(&game);
    assert!(chosen.heap == 3, "from_the_last_heap");
    assert!(nim_sum(&[1, 2, 3]) == 0, "leaves_zero");
    assert!(chosen.take == 1, "takes_one");
}

#[test]
fn a_losing_computer_takes_one_from_the_largest_heap() {
    // y: covers computer_move#losing
    let game = Game { heaps: vec![1, 4, 5] };
    let chosen = computer_move(&game);
    assert!(chosen.heap == 3, "from_the_largest_heap");
    assert!(chosen.take == 1, "takes_one");
}

#[test]
fn a_player_who_plays_perfectly_wins() {
    // y: covers play#playing, play#persons_turn, play#line_read, play#attempted.valid, play#computers_turn, play#game_over,
    // play#person_won
    let console = Console::fake(&["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"], false);
    let forever = Forever::fake();
    play(&mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_player_who_plays_carelessly_loses() {
    // y: covers play#computer_won
    let console = Console::fake(&["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"], false);
    let forever = Forever::fake();
    play(&mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers play#attempted.not_a_move, play#attempted.no_heap, play#attempted.none_taken, play#attempted.too_many,
    // play#closed, play!play_error.input_closed
    let console = Console::fake(&["one", "9 1", "1 0", "1 9"], false);
    let forever = Forever::fake();
    let ended = play(&mut console, &forever);
    let written = console.written();
    assert!(written.iter().any(|line| line == "Type a heap's number and how many to take, such as 2 3."), "told_the_form: Expected the player to be shown how a move is typed.");
    assert!(written.iter().any(|line| line == "There is no such heap."), "told_no_heap");
    assert!(written.iter().any(|line| line == "Take at least one counter."), "told_to_take");
    assert!(written.iter().any(|line| line == "That heap does not hold that many."), "told_too_many");
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers play!play_error.console
    let console = Console::fake(&[], true);
    let forever = Forever::fake();
    let ended = play(&mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}
