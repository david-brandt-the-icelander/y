//! Lowered from `y/src/nim.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

#[test]
fn a_move_is_read() {
    // y: covers parse_move > two > numbers, check > heap_exists'2lt > judged.fits, read_move
    let game = new_game();
    let chosen = read_move(&game, " 2  3 ").expect("when");
    assert!(chosen.heap == 2, "heap_two");
    assert!(chosen.take == 3, "three_counters");
}

#[test]
fn one_number_is_not_a_move() {
    // y: covers parse_move > not_two'lt, read_move > parsed'fails
    let game = new_game();
    let refused = read_move(&game, "3");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn two_words_are_not_a_move() {
    // y: covers parse_move > two > not_numbers'1
    let game = new_game();
    let refused = read_move(&game, "two three");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn there_is_no_fourth_heap() {
    // y: covers check > no_such_heap'2
    let game = new_game();
    let refused = read_move(&game, "4 1");
    assert!(refused == Err(MoveError::NoSuchHeap), "refused_as_no_heap");
}

#[test]
fn taking_nothing_is_not_a_move() {
    // y: covers check > heap_exists'2lt > judged.nothing_taken
    let game = new_game();
    let refused = read_move(&game, "1 0");
    assert!(refused == Err(MoveError::NothingTaken), "refused_as_nothing_taken");
}

#[test]
fn taking_too_many_is_not_a_move() {
    // y: covers check > heap_exists'2lt > judged.too_many
    let game = new_game();
    let refused = read_move(&game, "1 4");
    assert!(refused == Err(MoveError::TooMany), "refused_as_too_many");
}

#[test]
fn the_computer_finds_the_winning_move() {
    // y: covers computer_move > winnable > number'many > winning, counters_left > heap'many, nim_sum > heap'many
    let game = Game { heaps: vec![1, 2, 4] };
    let chosen = computer_move(&game);
    assert!(chosen.heap == 3, "from_the_last_heap");
    assert!(nim_sum(&[1, 2, 3]) == 0, "leaves_zero");
    assert!(chosen.take == 1, "takes_one");
}

#[test]
fn a_losing_computer_takes_one_from_the_largest_heap() {
    // y: covers computer_move > lost
    let game = Game { heaps: vec![1, 4, 5] };
    let chosen = computer_move(&game);
    assert!(chosen.heap == 3, "from_the_largest_heap");
    assert!(chosen.take == 1, "takes_one");
}

#[test]
fn a_player_who_plays_perfectly_wins() {
    // y: covers play > playing'many > persons_turn, play > playing'many > computers_turn
    let console = Console::fake(&["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_player_who_plays_carelessly_loses() {
    let console = Console::fake(&["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers persons_move > closed, persons_move > line_read > attempted.invalid, complaint > wrong.no_heap,
    // complaint > wrong.none_taken
    let console = Console::fake(&["one", "9 1", "1 0", "1 9"], false);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    let written = console.written();
    assert!(written.iter().any(|line| line == "Type a heap's number and how many to take, such as 2 3."), "told_the_form: Expected the player to be shown how a move is typed.");
    assert!(written.iter().any(|line| line == "There is no such heap."), "told_no_heap");
    assert!(written.iter().any(|line| line == "Take at least one counter."), "told_to_take");
    assert!(written.iter().any(|line| line == "That heap does not hold that many."), "told_too_many");
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers new_game, persons_move > typed'fails
    let console = Console::fake(&[], true);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn odd_lines_are_not_moves() {
    // y: covers parse_move > not_two'gt, parse_move > two > not_numbers'2
    let game = new_game();
    let three = read_move(&game, "1 2 3");
    let word_first = read_move(&game, "x 2");
    let word_second = read_move(&game, "2 x");
    assert!(three == Err(MoveError::NotTwoNumbers), "not_three");
    assert!(word_first == Err(MoveError::NotTwoNumbers), "not_word_first");
    assert!(word_second == Err(MoveError::NotTwoNumbers), "not_word_second");
}

#[test]
fn the_last_heap_and_heap_zero() {
    // y: covers check > no_such_heap'1, check > heap_exists'2eq > judged.nothing_taken,
    // check > heap_exists'2eq > judged.too_many
    let game = new_game();
    let zero = read_move(&game, "0 1");
    let nothing = read_move(&game, "3 0");
    let too_many = read_move(&game, "3 6");
    assert!(zero == Err(MoveError::NoSuchHeap), "no_heap_zero");
    assert!(nothing == Err(MoveError::NothingTaken), "nothing_taken");
    assert!(too_many == Err(MoveError::TooMany), "more_than_held");
}

#[test]
fn one_heap_and_none() {
    // y: covers counters_left > no_heaps, counters_left > heap, nim_sum > no_heaps, nim_sum > heap, shown > no_heaps,
    // shown > heap
    let one = Game { heaps: vec![5] };
    let none = Game { heaps: vec![] };
    let one_left = counters_left(&one);
    let one_sum = nim_sum(&one.heaps);
    let one_shown = shown(&one);
    let none_left = counters_left(&none);
    let none_sum = nim_sum(&none.heaps);
    let none_shown = shown(&none);
    assert!(one_left == 5, "five_left");
    assert!(one_sum == 5, "sum_five");
    assert!(one_shown == "  1: |||||", "one_line");
    assert!(none_left == 0, "nothing_left");
    assert!(none_sum == 0, "sum_zero");
    assert!(none_shown.is_empty(), "nothing_shown");
}

#[test]
fn the_computer_takes_a_lone_heap() {
    // y: covers computer_move > winnable > number > winning
    let game = Game { heaps: vec![3] };
    let chosen = computer_move(&game);
    assert!(chosen == Move { heap: 1, take: 3 }, "all_of_it");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers play > write_line'1'fails
    let console = Console::fake_failing_writes(&[], 0);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_draw_the_heaps_ends_play() {
    // y: covers shown > heap'many, play > playing > write_line'2'fails
    let console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_draw_the_heaps_again_ends_play() {
    // y: covers remove, play > playing'many > write_line'2'fails, persons_move > line_read > attempted.valid
    let console = Console::fake_failing_writes(&["1 1"], 3);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_ask_for_a_move_ends_play() {
    // y: covers play > playing > persons_turn > turn'fails, persons_move > write'fails
    let console = Console::fake_failing_writes(&[], 2);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_complain_ends_play() {
    // y: covers persons_move > line_read > attempted.invalid > write_line'fails, complaint > wrong.not_a_move
    let console = Console::fake_failing_writes(&["x"], 3);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_tell_the_computers_move_ends_play() {
    // y: covers play > playing'many > computers_turn > write_line'3'fails
    let console = Console::fake_failing_writes(&["1 1"], 4);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_the_computers_win_ends_play() {
    // y: covers play > playing'many > persons_turn > announce'fails,
    // play > playing'many > computers_turn > announce'fails
    let console = Console::fake_failing_writes(&["1 1"], 5);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_a_quick_win_ends_play() {
    // y: covers check > heap_exists'2eq > judged.fits, play > playing > persons_turn > announce'fails
    let console = Console::fake_failing_writes(&["1 2"], 3);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![2] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_a_win_ends_play() {
    // y: covers play > playing'many > persons_turn > turn'fails, complaint > wrong.too_many
    let console = Console::fake_failing_writes(&["1 1", "2 1"], 7);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_at_once_ends_play() {
    // y: covers play > nothing_to_play > announce'fails
    let console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![0] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_game_with_no_counters_is_over_at_once() {
    // y: covers play > nothing_to_play, announce > computer_won
    let console = Console::fake(&[], false);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![0] }, &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn one_move_can_win_a_game() {
    // y: covers play > playing > persons_turn, announce > person_won
    let console = Console::fake(&["1 2"], false);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![2] }, &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_computers_win_cannot_be_announced() {
    // y: covers announce > computer_won > write_line'1'fails
    let console = Console::fake_failing_writes(&[], 0);
    let said = announce(Player::Person, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_players_win_cannot_be_announced() {
    // y: covers announce > person_won > write_line'2'fails
    let console = Console::fake_failing_writes(&[], 0);
    let said = announce(Player::Computer, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}
