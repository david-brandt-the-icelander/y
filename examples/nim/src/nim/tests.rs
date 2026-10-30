//! Lowered from `y/src/nim.test.y`. Each test says, in a comment, the path its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

#[test]
fn a_new_game_has_every_heap_full() {
    // y: covers new_game
    let started = new_game();
    assert!(started.heaps == [3, 4, 5], "full");
}

#[test]
fn a_move_is_parsed() {
    // y: covers parse_move > two > numbers
    let parsed = parse_move(" 2  3 ").expect("when");
    assert!(parsed.heap == 2, "heap_two");
    assert!(parsed.take == 3, "three_counters");
}

#[test]
fn one_number_is_not_a_move() {
    // y: covers parse_move > not_two'lt
    let refused = parse_move("3");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn three_numbers_are_not_a_move() {
    // y: covers parse_move > not_two'gt
    let refused = parse_move("1 2 3");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn a_word_for_the_heap_is_not_a_move() {
    // y: covers parse_move > two > not_numbers'1
    let refused = parse_move("x 2");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn a_word_for_the_count_is_not_a_move() {
    // y: covers parse_move > two > not_numbers'2
    let refused = parse_move("2 x");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn there_is_no_heap_zero() {
    // y: covers check > no_such_heap'1
    let game = new_game();
    let refused = check(&game, &Move { heap: 0, take: 1 });
    assert!(refused == Err(MoveError::NoSuchHeap), "refused_as_no_heap");
}

#[test]
fn there_is_no_fourth_heap() {
    // y: covers check > no_such_heap'2
    let game = new_game();
    let refused = check(&game, &Move { heap: 4, take: 1 });
    assert!(refused == Err(MoveError::NoSuchHeap), "refused_as_no_heap");
}

#[test]
fn taking_nothing_is_not_a_move() {
    // y: covers check > heap_exists'2lt > judged.nothing_taken
    let game = new_game();
    let refused = check(&game, &Move { heap: 1, take: 0 });
    assert!(refused == Err(MoveError::NothingTaken), "refused_as_nothing_taken");
}

#[test]
fn taking_too_many_is_not_a_move() {
    // y: covers check > heap_exists'2lt > judged.too_many
    let game = new_game();
    let refused = check(&game, &Move { heap: 1, take: 4 });
    assert!(refused == Err(MoveError::TooMany), "refused_as_too_many");
}

#[test]
fn a_move_that_fits_is_allowed() {
    // y: covers check > heap_exists'2lt > judged.fits
    let game = new_game();
    let allowed = check(&game, &Move { heap: 2, take: 3 }).expect("when");
    assert!(allowed == Move { heap: 2, take: 3 }, "the_same_move");
}

#[test]
fn taking_nothing_from_the_last_heap_is_not_a_move() {
    // y: covers check > heap_exists'2eq > judged.nothing_taken
    let game = new_game();
    let refused = check(&game, &Move { heap: 3, take: 0 });
    assert!(refused == Err(MoveError::NothingTaken), "refused_as_nothing_taken");
}

#[test]
fn taking_too_many_from_the_last_heap_is_not_a_move() {
    // y: covers check > heap_exists'2eq > judged.too_many
    let game = new_game();
    let refused = check(&game, &Move { heap: 3, take: 6 });
    assert!(refused == Err(MoveError::TooMany), "refused_as_too_many");
}

#[test]
fn taking_the_whole_last_heap_is_allowed() {
    // y: covers check > heap_exists'2eq > judged.fits
    let game = new_game();
    let allowed = check(&game, &Move { heap: 3, take: 5 }).expect("when");
    assert!(allowed == Move { heap: 3, take: 5 }, "the_same_move");
}

#[test]
fn a_move_is_read() {
    // y: covers read_move
    let game = new_game();
    let chosen = read_move(&game, " 2  3 ").expect("when");
    assert!(chosen.heap == 2, "heap_two");
    assert!(chosen.take == 3, "three_counters");
}

#[test]
fn a_line_that_is_not_a_move_is_refused() {
    // y: covers read_move > parsed'fails
    let game = new_game();
    let refused = read_move(&game, "two three");
    assert!(refused == Err(MoveError::NotTwoNumbers), "refused_as_not_a_move");
}

#[test]
fn a_move_the_game_does_not_allow_is_refused() {
    let game = new_game();
    let refused = read_move(&game, "4 1");
    assert!(refused == Err(MoveError::NoSuchHeap), "refused_as_no_heap");
}

#[test]
fn a_move_takes_counters_from_its_heap() {
    // y: covers remove
    let mut game = new_game();
    remove(&mut game, &Move { heap: 2, take: 3 });
    assert!(game.heaps == [3, 1, 5], "one_left_in_heap_two");
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
fn the_computer_takes_a_lone_heap() {
    // y: covers computer_move > winnable > number > winning
    let game = Game { heaps: vec![3] };
    let chosen = computer_move(&game);
    assert!(chosen == Move { heap: 1, take: 3 }, "all_of_it");
}

#[test]
fn the_computer_finds_the_winning_move() {
    // y: covers computer_move > winnable > number'many > winning
    let game = Game { heaps: vec![1, 2, 4] };
    let chosen = computer_move(&game);
    assert!(chosen.heap == 3, "from_the_last_heap");
    assert!(nim_sum(&[1, 2, 3]) == 0, "leaves_zero");
    assert!(chosen.take == 1, "takes_one");
}

#[test]
fn no_heaps_leave_no_counters() {
    // y: covers counters_left > no_heaps
    let none = Game { heaps: vec![] };
    let left = counters_left(&none);
    assert!(left == 0, "nothing_left");
}

#[test]
fn a_lone_heap_is_all_that_is_left() {
    // y: covers counters_left > heap
    let one = Game { heaps: vec![5] };
    let left = counters_left(&one);
    assert!(left == 5, "five_left");
}

#[test]
fn the_counters_of_every_heap_are_counted() {
    // y: covers counters_left > heap'many
    let game = new_game();
    let left = counters_left(&game);
    assert!(left == 12, "twelve_left");
}

#[test]
fn no_heaps_sum_to_zero() {
    // y: covers nim_sum > no_heaps
    let sum = nim_sum(&[]);
    assert!(sum == 0, "sum_zero");
}

#[test]
fn a_lone_heap_sums_to_its_size() {
    // y: covers nim_sum > heap
    let sum = nim_sum(&[5]);
    assert!(sum == 5, "sum_five");
}

#[test]
fn heaps_are_summed_with_exclusive_or() {
    // y: covers nim_sum > heap'many
    let sum = nim_sum(&[3, 4, 5]);
    assert!(sum == 2, "sum_two");
}

#[test]
fn no_heaps_draw_nothing() {
    // y: covers shown > no_heaps
    let none = Game { heaps: vec![] };
    let picture = shown(&none);
    assert!(picture.is_empty(), "nothing_shown");
}

#[test]
fn a_lone_heap_is_drawn_on_one_line() {
    // y: covers shown > heap
    let one = Game { heaps: vec![5] };
    let picture = shown(&one);
    assert!(picture == "  1: |||||", "one_line");
}

#[test]
fn heaps_are_drawn_one_line_each() {
    // y: covers shown > heap'many
    let game = Game { heaps: vec![1, 2] };
    let picture = shown(&game);
    assert!(picture == "  1: |\n  2: ||", "two_lines");
}

#[test]
fn a_player_who_plays_perfectly_wins() {
    // y: covers play > playing'many > persons_turn
    let mut console = Console::fake(&["1 2", "1 1", "3 1", "3 1", "3 1", "3 1"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_player_who_plays_carelessly_loses() {
    // y: covers play > playing'many > computers_turn
    let mut console = Console::fake(&["1 1", "1 1", "2 1", "2 1", "2 1", "2 1"], false);
    let forever = Forever::fake();
    play(new_game(), &mut console, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    let mut console = Console::fake(&["one", "9 1", "1 0", "1 9"], false);
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
    let mut console = Console::fake(&[], true);
    let forever = Forever::fake();
    let ended = play(new_game(), &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers play > write_line'1'fails
    let mut console = Console::fake_failing_writes(&[], 0);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_draw_the_heaps_ends_play() {
    // y: covers play > playing > write_line'2'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_draw_the_heaps_again_ends_play() {
    // y: covers play > playing'many > write_line'2'fails
    let mut console = Console::fake_failing_writes(&["1 1"], 3);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_ask_for_a_move_ends_play() {
    // y: covers play > playing > persons_turn > turn'fails
    let mut console = Console::fake_failing_writes(&[], 2);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![3, 4, 5] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_fails_on_a_later_turn_ends_play() {
    // y: covers play > playing'many > persons_turn > turn'fails
    let mut console = Console::fake_failing_writes(&["1 1", "2 1"], 7);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_tell_the_computers_move_ends_play() {
    // y: covers play > playing'many > computers_turn > write_line'3'fails
    let mut console = Console::fake_failing_writes(&["1 1"], 4);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_the_computers_win_ends_play() {
    // y: covers play > playing'many > computers_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["1 1"], 5);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_a_win_ends_play() {
    // y: covers play > playing'many > persons_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["1 1", "3 1"], 7);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![1, 1, 1] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_a_quick_win_ends_play() {
    // y: covers play > playing > persons_turn > announce'fails
    let mut console = Console::fake_failing_writes(&["1 2"], 3);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![2] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_announce_at_once_ends_play() {
    // y: covers play > nothing_to_play > announce'fails
    let mut console = Console::fake_failing_writes(&[], 1);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![0] }, &mut console, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_game_with_no_counters_is_over_at_once() {
    // y: covers play > nothing_to_play
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![0] }, &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn one_move_can_win_a_game() {
    // y: covers play > playing > persons_turn
    let mut console = Console::fake(&["1 2"], false);
    let forever = Forever::fake();
    let ended = play(Game { heaps: vec![2] }, &mut console, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_move_cannot_be_asked_for() {
    // y: covers persons_move > write'fails
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&["1 1"], 0);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever);
    assert!(next == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_move_cannot_be_read() {
    // y: covers persons_move > typed'fails
    let mut game = new_game();
    let mut console = Console::fake(&[], true);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever);
    assert!(next == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_closed_console_has_no_move() {
    // y: covers persons_move > closed
    let mut game = new_game();
    let mut console = Console::fake(&[], false);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever);
    assert!(next == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_players_move_is_made() {
    // y: covers persons_move > line_read > attempted.valid
    let mut game = new_game();
    let mut console = Console::fake(&["2 3"], false);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever).expect("when");
    assert!(next == Player::Computer, "the_computers_turn");
    assert!(game.heaps == [3, 1, 5], "one_left_in_heap_two");
}

#[test]
fn a_player_is_told_what_is_wrong_and_asked_again() {
    // y: covers persons_move > line_read > attempted.invalid
    let mut game = new_game();
    let mut console = Console::fake(&["x"], false);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever).expect("when");
    let written = console.written();
    assert!(next == Player::Person, "the_players_turn_again");
    assert!(written.iter().any(|line| line == "Type a heap's number and how many to take, such as 2 3."), "told_the_form");
    assert!(game.heaps == [3, 4, 5], "nothing_taken");
}

#[test]
fn a_complaint_cannot_be_written() {
    // y: covers persons_move > line_read > attempted.invalid > write_line'fails
    let mut game = new_game();
    let mut console = Console::fake_failing_writes(&["x"], 1);
    let forever = Forever::fake();
    let next = persons_move(&mut game, &mut console, &forever);
    assert!(next == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn not_a_move_is_answered_with_the_form() {
    // y: covers complaint > wrong.not_a_move
    let said = complaint(MoveError::NotTwoNumbers);
    assert!(said == "Type a heap's number and how many to take, such as 2 3.", "the_form");
}

#[test]
fn no_such_heap_is_answered() {
    // y: covers complaint > wrong.no_heap
    let said = complaint(MoveError::NoSuchHeap);
    assert!(said == "There is no such heap.", "no_heap");
}

#[test]
fn nothing_taken_is_answered() {
    // y: covers complaint > wrong.none_taken
    let said = complaint(MoveError::NothingTaken);
    assert!(said == "Take at least one counter.", "take_one");
}

#[test]
fn too_many_is_answered() {
    // y: covers complaint > wrong.too_many
    let said = complaint(MoveError::TooMany);
    assert!(said == "That heap does not hold that many.", "not_that_many");
}

#[test]
fn the_computers_win_is_announced() {
    // y: covers announce > computer_won
    let mut console = Console::fake(&[], false);
    announce(Player::Person, &mut console).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("The computer took the last counter. The computer wins."), "announced");
}

#[test]
fn the_players_win_is_announced() {
    // y: covers announce > person_won
    let mut console = Console::fake(&[], false);
    announce(Player::Computer, &mut console).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You took the last counter. You win!"), "announced");
}

#[test]
fn a_computers_win_cannot_be_announced() {
    // y: covers announce > computer_won > write_line'1'fails
    let mut console = Console::fake_failing_writes(&[], 0);
    let said = announce(Player::Person, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_players_win_cannot_be_announced() {
    // y: covers announce > person_won > write_line'2'fails
    let mut console = Console::fake_failing_writes(&[], 0);
    let said = announce(Player::Computer, &mut console);
    assert!(said == Err(ConsoleError::fake_write_failure()), "failed_writing");
}
