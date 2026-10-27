//! Lowered from `y/src/hangman.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! A `?` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.

use super::*;

#[test]
fn a_letter_in_the_word_is_a_hit() {
    // y: covers guess#nothing_typed.else, guess#not_one_rune.else, guess#not_a_letter.else, guess#repeated.else,
    //    guess#in_word, shown_rune#known, shown_rune#known.else
    let game = new_game(Word::new("gallows".to_string()).expect("given"));
    let verdict = guess(&mut game, "l").expect("when");
    assert!(verdict == Verdict::Hit, "hit");
    assert!(masked(&game) == "_ _ l l _ _ _", "shown_twice");
}

#[test]
fn a_letter_not_in_the_word_is_a_miss() {
    // y: covers guess#in_word.else
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let verdict = guess(&mut game, "z").expect("when");
    assert!(verdict == Verdict::Miss, "miss");
    assert!(game.misses == vec![Rune('z')], "recorded");
}

#[test]
fn a_repeated_letter_is_refused() {
    // y: covers guess#repeated, guess!AlreadyGuessed
    let game = Game { word: Word::new("rope".to_string()).expect("given"), hits: BTreeSet::new(), misses: vec![Rune('z')] };
    let refused = guess(&mut game, "Z");
    assert!(refused == Err(GuessError::AlreadyGuessed { letter: Rune('z') }), "refused_as_repeated");
    assert!(game.misses.len() == 1, "no_new_miss");
}

#[test]
fn typing_nothing_is_not_a_guess() {
    // y: covers guess#nothing_typed, guess!Empty
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "   ");
    assert!(refused == Err(GuessError::Empty), "refused_as_empty");
}

#[test]
fn two_letters_are_not_a_guess() {
    // y: covers guess#not_one_rune, guess!NotOneLetter
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "ab");
    assert!(refused == Err(GuessError::NotOneLetter { text: "ab".to_string() }), "refused_as_two");
}

#[test]
fn a_digit_is_not_a_guess() {
    // y: covers guess#not_a_letter
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let refused = guess(&mut game, "7");
    assert!(refused == Err(GuessError::NotOneLetter { text: "7".to_string() }), "refused_as_digit");
}

#[test]
fn six_misses_hang_the_man() {
    // y: covers outcome#hanged
    let misses = vec![Rune('a'), Rune('b'), Rune('c'), Rune('d'), Rune('f'), Rune('g')];
    let game = Game { word: Word::new("rope".to_string()).expect("given"), hits: BTreeSet::new(), misses };
    let ending = outcome(&game);
    assert!(ending == Outcome::Lost, "lost");
}

#[test]
fn finding_every_letter_wins() {
    // y: covers outcome#hanged.else, outcome#solved
    let hits = BTreeSet::from([Rune('r'), Rune('o'), Rune('p'), Rune('e')]);
    let game = Game { word: Word::new("rope".to_string()).expect("given"), hits, misses: vec![Rune('a')] };
    let ending = outcome(&game);
    assert!(ending == Outcome::Won, "won");
}

#[test]
fn a_new_game_is_being_played() {
    // y: covers outcome#solved.else
    let game = new_game(Word::new("rope".to_string()).expect("given"));
    let ending = outcome(&game);
    assert!(ending == Outcome::Playing, "playing");
}

#[test]
fn the_empty_gallows() {
    // y: covers gallows#has_head.else, gallows#has_body.else, gallows#has_left_arm.else, gallows#has_right_arm.else,
    //    gallows#has_left_leg.else, gallows#has_right_leg.else
    let picture = gallows(0);
    let expected = ["  +---+", "  |   |", "      |", "      |", "      |", "      |", "========="].join("\n");
    assert!(picture == expected, "empty");
}

#[test]
fn the_full_gallows() {
    // y: covers gallows#has_head, gallows#has_body, gallows#has_left_arm, gallows#has_right_arm, gallows#has_left_leg,
    //    gallows#has_right_leg
    let picture = gallows(6);
    let expected = ["  +---+", "  |   |", "  O   |", " /|\\  |", " / \\  |", "      |", "========="].join("\n");
    assert!(picture == expected, "full");
}

#[test]
fn a_word_list_is_read_in_order() {
    // y: covers parse_word_list#blank, parse_word_list#blank.else, parse_word_list#empty.else
    let words = parse_word_list("rope  gallows").expect("when");
    let expected = vec![Word::new("rope".to_string()).expect("then"), Word::new("gallows".to_string()).expect("then")];
    assert!(words == expected, "both_in_order");
}

#[test]
fn an_empty_word_list_is_refused() {
    // y: covers parse_word_list#empty, parse_word_list!Empty
    let refused = parse_word_list("   ");
    assert!(refused == Err(WordListError::Empty), "refused_as_empty");
}

#[test]
fn a_short_word_is_refused() {
    // y: covers parse_word_list!NotAWord
    let refused = parse_word_list("rope cat");
    assert!(refused == Err(WordListError::NotAWord { text: "cat".to_string() }), "refused_naming_it");
}

#[test]
fn a_player_who_finds_every_letter_wins() {
    // y: covers play#playing, play#playing.else, play#closed.else, play#won
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let console = Console::fake(&["r", "o", "p", "e"], false);
    let random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("You win! The word was rope."), "announced");
}

#[test]
fn a_player_who_misses_six_times_is_hanged() {
    // y: covers play#won.else
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let console = Console::fake(&["a", "b", "c", "d", "f", "g"], false);
    let random = Random::seeded(7);
    let forever = Forever::fake();
    play(&words, &mut console, &mut random, &forever).expect("when");
    let last = console.written().last().map(String::as_str);
    assert!(last == Some("Hanged! The word was rope."), "announced");
}

#[test]
fn a_closed_console_ends_the_game() {
    // y: covers play#closed, play!InputClosed
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let console = Console::fake(&["r"], false);
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::InputClosed), "closed");
}

#[test]
fn a_failing_console_ends_the_game() {
    // y: covers play!Console
    let words = vec![Word::new("rope".to_string()).expect("given")];
    let console = Console::fake(&[], true);
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let ended = play(&words, &mut console, &mut random, &forever);
    assert!(ended == Err(PlayError::Console { error: ConsoleError::fake_read_failure() }), "failed_reading");
}
