//! Lowered from `y/src/hangman.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::console_play::*;
use super::*;

#[test]
fn a_letter_in_the_word_is_a_hit() {
    // y: covers guess > judged.in_word
    let mut game = new_game(Word::new(YString::new("gallows")).expect("given"));
    let verdict = guess(&mut game, &YString::new("l")).expect("when");
    assert!(verdict == Verdict::Hit, "hit");
    assert!(game.hits.contains(&Rune::from_scalar('l')), "recorded");
}

#[test]
fn a_letter_not_in_the_word_is_a_miss() {
    // y: covers guess > judged.not_in_word
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    let verdict = guess(&mut game, &YString::new("z")).expect("when");
    assert!(verdict == Verdict::Miss, "miss");
    assert!(game.misses == List::from([Rune::from_scalar('z')]), "recorded");
}

#[test]
fn a_letter_already_found_is_refused() {
    // y: covers guess > judged.repeated'1
    let mut game = Game {
        word: Word::new(YString::new("rope")).expect("given"),
        hits: Set::from_iter([Rune::from_scalar('r')]),
        misses: List::new(),
    };
    let refused = guess(&mut game, &YString::new("r"));
    assert!(refused == Err(GuessError::AlreadyGuessed), "refused_as_repeated");
    assert!(game.hits.count() == 1, "no_new_hit");
    assert!(game.misses.is_empty(), "no_new_miss");
}

#[test]
fn a_repeated_letter_is_refused() {
    // y: covers guess > judged.repeated'2
    let mut game = Game {
        word: Word::new(YString::new("rope")).expect("given"),
        hits: Set::new(),
        misses: List::from([Rune::from_scalar('z')]),
    };
    let refused = guess(&mut game, &YString::new("Z"));
    assert!(refused == Err(GuessError::AlreadyGuessed), "refused_as_repeated");
    assert!(game.misses.count() == 1, "no_new_miss");
}

#[test]
fn typing_nothing_is_not_a_guess() {
    // y: covers guess > judged.nothing_typed
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    let refused = guess(&mut game, &YString::new("   "));
    assert!(refused == Err(GuessError::Empty), "refused_as_empty");
}

#[test]
fn two_letters_are_not_a_guess() {
    // y: covers guess > judged.not_one_rune
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    let refused = guess(&mut game, &YString::new("ab"));
    assert!(refused == Err(GuessError::NotOneLetter), "refused_as_two");
}

#[test]
fn a_digit_is_not_a_guess() {
    // y: covers guess > judged.not_a_letter
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    let refused = guess(&mut game, &YString::new("7"));
    assert!(refused == Err(GuessError::NotOneLetter), "refused_as_digit");
}

#[test]
fn six_misses_hang_the_man() {
    // y: covers outcome_of > judged.hanged
    let misses = List::from([
        Rune::from_scalar('a'),
        Rune::from_scalar('b'),
        Rune::from_scalar('c'),
        Rune::from_scalar('d'),
        Rune::from_scalar('f'),
        Rune::from_scalar('g'),
    ]);
    let game = Game { word: Word::new(YString::new("rope")).expect("given"), hits: Set::new(), misses };
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Lost, "lost");
}

#[test]
fn finding_every_letter_wins() {
    // y: covers outcome_of > judged.solved
    let hits = Set::from_iter([
        Rune::from_scalar('r'),
        Rune::from_scalar('o'),
        Rune::from_scalar('p'),
        Rune::from_scalar('e'),
    ]);
    let game = Game {
        word: Word::new(YString::new("rope")).expect("given"),
        hits,
        misses: List::from([Rune::from_scalar('a')]),
    };
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Won, "won");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers outcome_of > judged.undecided
    let game = new_game(Word::new(YString::new("rope")).expect("given"));
    let ending = outcome_of(&game);
    assert!(ending == Outcome::Playing, "playing");
}

#[test]
fn a_new_game_has_nothing_guessed() {
    // y: covers new_game
    let secret = Word::new(YString::new("rope")).expect("given");
    let started = new_game(secret);
    assert!(started.hits.is_empty(), "nothing_found");
    assert!(started.misses.is_empty(), "nothing_missed");
}

#[test]
fn a_found_letter_shows_wherever_it_occurs() {
    // y: covers masked > letter'many
    let game = Game {
        word: Word::new(YString::new("gallows")).expect("given"),
        hits: Set::from_iter([Rune::from_scalar('l')]),
        misses: List::new(),
    };
    let shown = masked(&game);
    assert!(shown == "_ _ l l _ _ _", "shown_twice");
}

#[test]
fn a_found_letter_is_shown() {
    // y: covers shown_rune > known
    let shown = shown_rune(Rune::from_scalar('l'), &Set::from_iter([Rune::from_scalar('l')]));
    assert!(shown == Rune::from_scalar('l'), "itself");
}

#[test]
fn a_letter_not_found_is_hidden() {
    // y: covers shown_rune > unknown
    let shown = shown_rune(Rune::from_scalar('l'), &Set::from_iter([Rune::from_scalar('r')]));
    assert!(shown == Rune::from_scalar('_'), "hidden");
}

#[test]
fn the_empty_gallows() {
    // y: covers gallows
    let expected = ["  +---+", "  |   |", "      |", "      |", "      |", "      |", "========="].join("\n");
    let picture = gallows(0);
    assert!(picture == expected.as_str(), "empty");
}

#[test]
fn the_full_gallows() {
    // y: covers gallows
    let expected = ["  +---+", "  |   |", "  O   |", " /|\\  |", " / \\  |", "      |", "========="].join("\n");
    let picture = gallows(6);
    assert!(picture == expected.as_str(), "full");
}

#[test]
fn a_part_is_drawn_once_enough_misses_are_made() {
    // y: covers part > reached
    let drawn = part(3, 3, &YString::new("/"));
    assert!(drawn == "/", "marked");
}

#[test]
fn a_part_is_blank_until_enough_misses_are_made() {
    // y: covers part > not_yet
    let drawn = part(2, 3, &YString::new("/"));
    assert!(drawn == " ", "blank");
}

#[test]
fn a_word_list_is_read_in_order() {
    // y: covers parse_word_list > piece'many > has_text > checked.valid > has_words
    let words = parse_word_list(&YString::new("rope gallows")).expect("when");
    assert!(
        words
            == List::from([
                Word::new(YString::new("rope")).expect("then"),
                Word::new(YString::new("gallows")).expect("then")
            ]),
        "both_in_order"
    );
}

#[test]
fn spaces_after_the_last_word_are_allowed() {
    // y: covers parse_word_list > piece'many > blank > has_words
    let words = parse_word_list(&YString::new("rope  ")).expect("when");
    assert!(words == List::from([Word::new(YString::new("rope")).expect("then")]), "the_word");
}

#[test]
fn a_list_of_one_word_is_read() {
    // y: covers parse_word_list > piece > has_text > checked.valid > has_words
    let words = parse_word_list(&YString::new("rope")).expect("when");
    assert!(words == List::from([Word::new(YString::new("rope")).expect("then")]), "the_word");
}

#[test]
fn an_empty_word_list_is_refused() {
    // y: covers parse_word_list > piece'many > blank > empty
    let refused = parse_word_list(&YString::new("   "));
    assert!(refused == Err(WordListError::Empty), "refused_as_empty");
}

#[test]
fn an_empty_text_is_refused() {
    // y: covers parse_word_list > piece > blank > empty
    let refused = parse_word_list(&YString::new(""));
    assert!(refused == Err(WordListError::Empty), "refused_as_empty");
}

#[test]
fn a_short_word_is_refused() {
    // y: covers parse_word_list > piece'many > has_text > checked.invalid
    let refused = parse_word_list(&YString::new("rope cat"));
    assert!(refused == Err(WordListError::NotAWord), "refused_as_not_a_word");
}

#[test]
fn a_list_of_one_short_word_is_refused() {
    // y: covers parse_word_list > piece > has_text > checked.invalid
    let refused = parse_word_list(&YString::new("cat"));
    assert!(refused == Err(WordListError::NotAWord), "refused_as_not_a_word");
}

#[test]
fn a_turn_shows_the_game_and_asks_for_a_guess() {
    // y: covers show_turn
    let game = Game {
        word: Word::new(YString::new("rope")).expect("given"),
        hits: Set::from_iter([Rune::from_scalar('o')]),
        misses: List::from([Rune::from_scalar('z')]),
    };
    let mut console = Console::fake(&[], false);
    show_turn(&game, &mut console).expect("when");
    let written = console.written();
    assert!(written.iter().any(|line| line == "Word:   _ o _ _"), "word_shown");
    assert!(written.iter().any(|line| line == "Missed: z"), "missed_shown");
    assert!(written.last().map(String::as_str) == Some("Guess:  "), "asked");
}

#[test]
fn a_turn_cannot_be_shown_after_no_lines() {
    // y: covers show_turn > write_line'1'fails
    let game = new_game(Word::new(YString::new("rope")).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 0);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_one_lines() {
    // y: covers show_turn > write_line'2'fails
    let game = new_game(Word::new(YString::new("rope")).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_two_lines() {
    // y: covers show_turn > write_line'3'fails
    let game = new_game(Word::new(YString::new("rope")).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 2);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_turn_cannot_be_shown_after_three_lines() {
    // y: covers show_turn > write'fails
    let game = new_game(Word::new(YString::new("rope")).expect("given"));
    let mut console = Console::fake_failing_writes(&[], 3);
    let shown = show_turn(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_hit_is_answered_yes() {
    // y: covers reply > said.hit
    let answer = reply(&Ok(Verdict::Hit), &YString::new("r"));
    assert!(answer == "Yes.", "yes");
}

#[test]
fn a_miss_is_answered_no() {
    // y: covers reply > said.miss
    let answer = reply(&Ok(Verdict::Miss), &YString::new("z"));
    assert!(answer == "No.", "no");
}

#[test]
fn typing_nothing_is_answered_with_a_request() {
    // y: covers reply > said.typed_nothing
    let answer = reply(&Err(GuessError::Empty), &YString::new(""));
    assert!(answer == "Type a letter.", "asked_for_a_letter");
}

#[test]
fn typing_other_than_a_letter_is_answered_with_what_was_typed() {
    // y: covers reply > said.typed_other
    let answer = reply(&Err(GuessError::NotOneLetter), &YString::new("ab"));
    assert!(answer == "\"ab\" is not one letter.", "told_not_one");
}

#[test]
fn a_repeated_letter_is_answered_with_the_letter() {
    // y: covers reply > said.typed_again
    let answer = reply(&Err(GuessError::AlreadyGuessed), &YString::new("r"));
    assert!(answer == "You have already guessed r.", "told_repeated");
}

#[test]
fn a_win_is_announced() {
    // y: covers show_end > won
    let hits = Set::from_iter([
        Rune::from_scalar('r'),
        Rune::from_scalar('o'),
        Rune::from_scalar('p'),
        Rune::from_scalar('e'),
    ]);
    let game = Game { word: Word::new(YString::new("rope")).expect("given"), hits, misses: List::new() };
    let mut console = Console::fake(&[], false);
    show_end(&game, &mut console).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("You win! The word was rope."), "announced");
}

#[test]
fn a_hanging_is_announced() {
    // y: covers show_end > hanged
    let misses = List::from([
        Rune::from_scalar('a'),
        Rune::from_scalar('b'),
        Rune::from_scalar('c'),
        Rune::from_scalar('d'),
        Rune::from_scalar('f'),
        Rune::from_scalar('g'),
    ]);
    let game = Game { word: Word::new(YString::new("rope")).expect("given"), hits: Set::new(), misses };
    let mut console = Console::fake(&[], false);
    show_end(&game, &mut console).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("Hanged! The word was rope."), "announced");
}

#[test]
fn an_end_cannot_be_shown_without_the_gallows() {
    // y: covers show_end > write_line'1'fails
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    guess(&mut game, &YString::new("r")).expect("given");
    guess(&mut game, &YString::new("o")).expect("given");
    guess(&mut game, &YString::new("p")).expect("given");
    guess(&mut game, &YString::new("e")).expect("given");
    let mut console = Console::fake_failing_writes(&[], 0);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_win_cannot_be_announced() {
    // y: covers show_end > won > write_line'2'fails
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    guess(&mut game, &YString::new("r")).expect("given");
    guess(&mut game, &YString::new("o")).expect("given");
    guess(&mut game, &YString::new("p")).expect("given");
    guess(&mut game, &YString::new("e")).expect("given");
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_hanging_cannot_be_announced() {
    // y: covers show_end > hanged > write_line'3'fails
    let mut game = new_game(Word::new(YString::new("rope")).expect("given"));
    guess(&mut game, &YString::new("a")).expect("given");
    guess(&mut game, &YString::new("b")).expect("given");
    guess(&mut game, &YString::new("c")).expect("given");
    guess(&mut game, &YString::new("d")).expect("given");
    guess(&mut game, &YString::new("f")).expect("given");
    guess(&mut game, &YString::new("g")).expect("given");
    let mut console = Console::fake_failing_writes(&[], 1);
    let shown = show_end(&game, &mut console);
    assert!(shown == Err(ConsoleError::fake_write_failure()), "failed_writing");
}

#[test]
fn a_word_of_one_letter_is_found_with_one_guess() {
    // y: covers play > playing > line_read
    let words = List::from([Word::new(YString::new("aaaa")).expect("given")]);
    let mut console = Console::fake(&["a"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    assert!(console.written().last().map(String::as_str) == Some("You win! The word was aaaa."), "won");
}

#[test]
fn a_player_who_finds_every_letter_wins() {
    // y: covers play > playing'many > line_read
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&["r", "o", "p", "e"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You win! The word was rope."), "announced");
}

#[test]
fn a_player_who_misses_six_times_is_hanged() {
    // y: covers play > playing'many > line_read
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&["a", "b", "c", "d", "f", "g"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("Hanged! The word was rope."), "announced");
}

#[test]
fn a_player_is_told_what_was_wrong() {
    // y: covers play > playing'many > line_read
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&["", "ab", "r", "r", "o", "p", "e"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let written = console.written();
    assert!(
        written.iter().any(|line| line == "Type a letter."),
        "told_to_type: Expected the player to be told to type a letter."
    );
    assert!(written.iter().any(|line| line == "\"ab\" is not one letter."), "told_not_one");
    assert!(written.iter().any(|line| line == "You have already guessed r."), "told_repeated");
    assert!(written.last().map(String::as_str) == Some("You win! The word was rope."), "still_won");
}

#[test]
fn a_console_that_cannot_greet_ends_play() {
    // y: covers play > write_line'1'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&[], 0);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_the_first_turn_ends_play() {
    // y: covers play > playing > show_turn'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&[], 1);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers play > playing > typed'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&[], true);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_console_closed_at_once_ends_play() {
    // y: covers play > playing > closed
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&[], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_console_that_cannot_reply_to_the_first_guess_ends_play() {
    // y: covers play > playing > line_read > write_line'2'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&["r"], 5);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_a_quick_end_ends_play() {
    // y: covers play > playing > line_read > show_end'fails
    let words = List::from([Word::new(YString::new("aaaa")).expect("given")]);
    let mut console = Console::fake_failing_writes(&["a"], 6);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_a_later_turn_ends_play() {
    // y: covers play > playing'many > show_turn'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&["x"], 6);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_read_a_later_guess_ends_play() {
    // y: covers play > playing'many > typed'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_reads(&["r"]);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}

#[test]
fn a_closed_console_ends_the_game() {
    // y: covers play > playing'many > closed
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake(&["r"], false);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_console_that_cannot_reply_to_a_later_guess_ends_play() {
    // y: covers play > playing'many > line_read > write_line'2'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&["r", "o"], 10);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}

#[test]
fn a_console_that_cannot_show_the_end_ends_play() {
    // y: covers play > playing'many > line_read > show_end'fails
    let words = List::from([Word::new(YString::new("rope")).expect("given")]);
    let mut console = Console::fake_failing_writes(&["r", "o", "p", "e"], 21);
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_write_failure() }), "failed_writing");
}
