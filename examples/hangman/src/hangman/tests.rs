//! Lowered from `y/src/hangman.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

#[test]
fn a_letter_in_the_word_is_a_hit() {
    // y: covers guess > judged.in_word, masked > letter'many, shown_rune > known, shown_rune > unknown
    let mut game = new_game(Word::new("gallows".to_string()).expect("given"));
    let verdict = guess(&mut game, "l").expect("when");
    assert!(verdict == Verdict::Hit, "hit");
    assert!(masked(&game) == "_ _ l l _ _ _", "shown_twice");
}

#[test]
fn a_letter_not_in_the_word_is_a_miss() {
    // y: covers guess > judged.not_in_word
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    let verdict = guess(&mut game, "z").expect("when");
    assert!(verdict == Verdict::Miss, "miss");
    assert!(game.misses == vec![Rune('z')], "recorded");
}

#[test]
fn a_repeated_letter_is_refused() {
    // y: covers guess > judged.repeated'2
    let mut game = Game { word: Word::new("rope".to_string()).expect("given"), hits: BTreeSet::new(), misses: vec![Rune('z')] };
    let refused = guess(&mut game, "Z");
    assert!(refused == Err(GuessError::AlreadyGuessed), "refused_as_repeated");
    assert!(game.misses.len() == 1, "no_new_miss");
}

#[test]
fn typing_nothing_is_not_a_guess() {
    // y: covers guess > judged.nothing_typed
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "   ");
    assert!(refused == Err(GuessError::Empty), "refused_as_empty");
}

#[test]
fn two_letters_are_not_a_guess() {
    // y: covers guess > judged.not_one_rune
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "ab");
    assert!(refused == Err(GuessError::NotOneLetter), "refused_as_two");
}

#[test]
fn a_digit_is_not_a_guess() {
    // y: covers guess > judged.not_a_letter
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "7");
    assert!(refused == Err(GuessError::NotOneLetter), "refused_as_digit");
}

#[test]
fn six_misses_hang_the_man() {
    // y: covers outcome_of > judged.hanged
    let misses = vec![Rune('a'), Rune('b'), Rune('c'), Rune('d'), Rune('f'), Rune('g')];
    let game = Game { word: Word::new("rope".to_string()).expect("given"), hits: BTreeSet::new(), misses };
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Lost, "lost");
}

#[test]
fn finding_every_letter_wins() {
    // y: covers outcome_of > judged.solved
    let hits = BTreeSet::from([Rune('r'), Rune('o'), Rune('p'), Rune('e')]);
    let game = Game { word: Word::new("rope".to_string()).expect("given"), hits, misses: vec![Rune('a')] };
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Won, "won");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers new_game, outcome_of > judged.undecided
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Playing, "playing");
}

#[test]
fn the_empty_gallows() {
    // y: covers gallows, part > not_yet
    let picture = gallows(0);
    let expected = ["  +---+", "  |   |", "      |", "      |", "      |", "      |", "========="].join("\n");
    assert!(picture == expected, "empty");
}

#[test]
fn the_full_gallows() {
    // y: covers part > reached
    let picture = gallows(6);
    let expected = ["  +---+", "  |   |", "  O   |", " /|\\  |", " / \\  |", "      |", "========="].join("\n");
    assert!(picture == expected, "full");
}

#[test]
fn a_word_list_is_read_in_order() {
    // y: covers parse_word_list > piece'many > has_text > checked.valid > has_words,
    // parse_word_list > piece'many > blank > has_words
    let words = parse_word_list("rope  gallows").expect("when");
    let expected = vec![Word::new("rope".to_string()).expect("then"), Word::new("gallows".to_string()).expect("then")];
    assert!(words == expected, "both_in_order");
}

#[test]
fn an_empty_word_list_is_refused() {
    // y: covers parse_word_list > piece'many > blank > empty
    let refused = parse_word_list("   ");
    assert!(refused == Err(WordListError::Empty), "refused_as_empty");
}

#[test]
fn a_short_word_is_refused() {
    // y: covers parse_word_list > piece'many > has_text > checked.invalid
    let refused = parse_word_list("rope cat");
    assert!(refused == Err(WordListError::NotAWord), "refused_as_not_a_word");
}

#[test]
fn a_player_who_finds_every_letter_wins() {
    // y: covers play > playing'many > line_read
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&["r", "o", "p", "e"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You win! The word was rope."), "announced");
}

#[test]
fn a_player_who_misses_six_times_is_hanged() {
    // y: covers show_end > hanged
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&["a", "b", "c", "d", "f", "g"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("Hanged! The word was rope."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers guess > judged.repeated'1, reply > said.typed_nothing, reply > said.typed_other,
    // reply > said.typed_again
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&["", "ab", "r", "r", "o", "p", "e"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let written = console.written();
    assert!(written.iter().any(|line| line == "Type a letter."), "told_to_type: Expected the player to be told to type a letter.");
    assert!(written.iter().any(|line| line == "\"ab\" is not one letter."), "told_not_one");
    assert!(written.iter().any(|line| line == "You have already guessed r."), "told_repeated");
    assert!(written.last().map(String::as_str) == Some("You win! The word was rope."), "still_won");
}

#[test]
fn a_closed_console_ends_the_game() {
    // y: covers play > playing'many > closed
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&["r"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers play > playing > typed'fails, show_turn
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&[], true);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_list_of_one_piece_is_read() {
    // y: covers parse_word_list > piece > has_text > checked.invalid,
    // parse_word_list > piece > has_text > checked.valid > has_words, parse_word_list > piece > blank > empty
    let one = parse_word_list("rope");
    let blank = parse_word_list("");
    let short = parse_word_list("cat");
    assert!(one == Ok(vec![Word::new("rope".to_string()).expect("then")]), "the_word");
    assert!(blank == Err(WordListError::Empty), "none");
    assert!(short == Err(WordListError::NotAWord), "refused");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers play > write_line'1'fails
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&[], 0);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_the_first_turn_ends_play() {
    // y: covers play > playing > show_turn'fails
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&[], 1);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_a_later_turn_ends_play() {
    // y: covers play > playing'many > show_turn'fails, reply > said.miss
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&["x"], 6);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_reply_to_the_first_guess_ends_play() {
    // y: covers play > playing > line_read > write_line'2'fails, reply > said.hit
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&["r"], 5);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_reply_to_a_later_guess_ends_play() {
    // y: covers play > playing'many > line_read > write_line'2'fails
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&["r", "o"], 10);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_a_quick_end_ends_play() {
    // y: covers play > playing > line_read > show_end'fails
    let words = vec![Word::new("aaaa".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&["a"], 6);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_the_end_ends_play() {
    // y: covers play > playing'many > line_read > show_end'fails
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_writes(&["r", "o", "p", "e"], 21);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_word_of_one_letter_is_found_with_one_guess() {
    // y: covers play > playing > line_read, show_end > won
    let words = vec![Word::new("aaaa".to_string()).expect("given")];
    let mut console = Console::fake(&["a"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended.is_ok(), "played");
    assert!(console.written().last().map(String::as_str) == Some("You win! The word was aaaa."), "won");
}

#[test]
fn a_console_closed_at_once_ends_play() {
    // y: covers play > playing > closed
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake(&[], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_console_that_cannot_read_a_later_guess_ends_play() {
    // y: covers play > playing'many > typed'fails
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let mut console = Console::fake_failing_reads(&["r"]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_turn_cannot_be_shown_after_no_lines() {
    // y: covers show_turn > write_line'1'fails
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 0);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_one_lines() {
    // y: covers show_turn > write_line'2'fails
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_two_lines() {
    // y: covers show_turn > write_line'3'fails
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 2);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_three_lines() {
    // y: covers show_turn > write'fails
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 3);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn an_end_cannot_be_shown_without_the_gallows() {
    // y: covers show_end > write_line'1'fails
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    guess(&mut game, "r").expect("given");
    guess(&mut game, "o").expect("given");
    guess(&mut game, "p").expect("given");
    guess(&mut game, "e").expect("given");
    let mut console = Console::fake_failing_writes(&[], 0);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_win_cannot_be_announced() {
    // y: covers show_end > won > write_line'2'fails
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    guess(&mut game, "r").expect("given");
    guess(&mut game, "o").expect("given");
    guess(&mut game, "p").expect("given");
    guess(&mut game, "e").expect("given");
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_hanging_cannot_be_announced() {
    // y: covers show_end > hanged > write_line'3'fails
    let mut game = new_game(Word::new("rope".to_string()).expect("given"));
    guess(&mut game, "a").expect("given");
    guess(&mut game, "b").expect("given");
    guess(&mut game, "c").expect("given");
    guess(&mut game, "d").expect("given");
    guess(&mut game, "f").expect("given");
    guess(&mut game, "g").expect("given");
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}
