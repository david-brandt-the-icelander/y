//! Lowered from `y/src/snake.test.y`. Each test says, in a comment, the outcomes its Y source declares it covers.
//! A test of `play` runs it with `block_on`: it holds a fake window, which never suspends.

use super::window_play::*;
use super::*;
use std::time::Duration;
use y_window::block_on;

/// A game with a snake one cell long in the middle of the board, moving one way, and the food in the top left corner.
///
/// - `heading`: the way the snake moves.
/// - Returns the game.
fn moving(heading) {
    Game {
        head: Cell { column: 10, row: 7 },
        tail: vec![],
        heading,
        wanted: heading,
        food: Cell { column: 0, row: 0 },
        eaten: 0,
        standing: Outcome::Playing,
    }
}

#[test]
fn a_new_game_puts_the_food_in_a_free_cell() {
    // y: covers new_game, free_cells > row'many > column'many > vacant, free_cells > row'many > column'many > taken'1,
    // free_cells > row'many > column'many > taken'2, place_food > room
    let random = Random::seeded(7);
    let started = new_game(&mut random);
    assert!(started.tail.len() == 2, "three_long");
    assert!(started.heading == Direction::Right, "moving_right");
    assert!(started.food != started.head && !started.tail.contains(&started.food), "food_off_the_snake");
}

#[test]
fn the_snake_moves_one_cell_the_way_it_heads() {
    // y: covers ahead_of > going.up, ahead_of > going.left
    let up = moving(Direction::Up);
    let down = moving(Direction::Down);
    let left = moving(Direction::Left);
    let right = moving(Direction::Right);
    let random = Random::seeded(7);
    advance(&mut up, &mut random);
    advance(&mut down, &mut random);
    advance(&mut left, &mut random);
    advance(&mut right, &mut random);
    assert!(up.head == Cell { column: 10, row: 6 }, "moved_up");
    assert!(down.head == Cell { column: 10, row: 8 }, "moved_down");
    assert!(left.head == Cell { column: 9, row: 7 }, "moved_left");
    assert!(right.head == Cell { column: 11, row: 7 }, "moved_right");
    assert!(right.standing == Outcome::Playing, "still_playing");
}

#[test]
fn the_tail_follows_the_head() {
    // y: covers advance > moved.on, ahead_of > going.right, on_board
    let game = moving(Direction::Right);
    game.tail = vec![Cell { column: 9, row: 7 }, Cell { column: 8, row: 7 }];
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.tail == vec![Cell { column: 10, row: 7 }, Cell { column: 9, row: 7 }], "followed");
}

#[test]
fn keys_turn_the_snake() {
    // y: covers turned > arrow.up'1, turned > arrow.down'2, turned > arrow.right'2, opposite > going.up,
    // opposite > going.down, opposite > going.left
    let heading_right = moving(Direction::Right);
    let heading_left = moving(Direction::Left);
    let heading_down = moving(Direction::Down);
    let heading_up = moving(Direction::Up);
    let up = turned(&heading_right, Key::Up);
    let down = turned(&heading_left, Key::S);
    let left = turned(&heading_down, Key::Left);
    let right = turned(&heading_up, Key::D);
    assert!(up == Direction::Up, "turned_up");
    assert!(down == Direction::Down, "turned_down");
    assert!(left == Direction::Left, "turned_left");
    assert!(right == Direction::Right, "turned_right");
}

#[test]
fn the_snake_does_not_turn_back_on_itself() {
    // y: covers turned > arrow.left'1, turned > arrow.other, unless_reversing > reversing, opposite > going.right
    let game = moving(Direction::Right);
    game.wanted = Direction::Up;
    let back = turned(&game, Key::Left);
    let other = turned(&game, Key::Space);
    assert!(back == Direction::Up, "Expected the turn the player made before to stand.");
    assert!(other == Direction::Up, "unchanged");
}

#[test]
fn the_wall_ends_the_game() {
    // y: covers advance > moved.into_wall, banner > shown.crashed
    let game = moving(Direction::Right);
    game.head = Cell { column: 19, row: 7 };
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Crashed, "crashed");
    assert!(game.head == Cell { column: 19, row: 7 }, "not_moved");
    assert!(banner(&game, 0) == "Crashed into the wall with 0. Space plays again.", "told");
}

#[test]
fn the_snake_cannot_bite_itself() {
    // y: covers advance > moved.into_itself, banner > shown.bitten
    let game = moving(Direction::Left);
    game.head = Cell { column: 5, row: 5 };
    game.tail = vec![Cell { column: 6, row: 5 }, Cell { column: 6, row: 6 }, Cell { column: 5, row: 6 }, Cell { column: 4, row: 6 }];
    game.wanted = Direction::Down;
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Bitten, "bitten");
    assert!(banner(&game, 0) == "Bit itself with 0. Space plays again.", "told");
}

#[test]
fn the_snake_may_follow_its_own_tail() {
    // y: covers ahead_of > going.down
    let game = moving(Direction::Left);
    game.head = Cell { column: 5, row: 5 };
    game.tail = vec![Cell { column: 6, row: 5 }, Cell { column: 6, row: 6 }, Cell { column: 5, row: 6 }];
    game.wanted = Direction::Down;
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Playing, "still_playing");
    assert!(game.head == Cell { column: 5, row: 6 }, "moved");
}

#[test]
fn eating_grows_the_snake() {
    // y: covers advance > moved.onto_food
    let game = moving(Direction::Left);
    game.food = Cell { column: 9, row: 7 };
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.tail == vec![Cell { column: 10, row: 7 }], "grown");
    assert!(game.eaten == 1, "scored");
    assert!(game.food != game.head && !game.tail.contains(&game.food), "new_food");
    assert!(banner(&game, 1) == "Score 1   Best 1   Arrows steer, Escape quits", "told");
}

#[test]
fn filling_the_board_ends_the_game() {
    // y: covers place_food > full, banner > shown.filled
    let corner = Cell { column: 0, row: 0 };
    let game = moving(Direction::Left);
    game.head = Cell { column: 1, row: 0 };
    game.tail = vec![corner];
    let rest = free_cells(&game);
    game.tail = rest;
    game.food = corner;
    let random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Filled, "filled");
    assert!(banner(&game, 298) == "Filled the board with 1! Space plays again.", "told");
}

#[test]
fn the_game_plays_until_the_window_closes() {
    // y: covers draw > part'many, play > open'many > going_on'1, steer > no_keys, catch_up > due'2eq
    let window = Window::fake(&[&[], &[], &[]]);
    let clock = Clock::fake(Duration::from_millis(150));
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.frames_shown() == 3, "three_frames");
    assert!(window.written().last().map(String::as_str) == Some("Score 0   Best 0   Arrows steer, Escape quits"), "drawn");
    assert!(best == 0, "no_score");
}

#[test]
fn space_starts_the_next_game() {
    // y: covers play > open'many > again, play > open'many > going_on'2
    let window = Window::fake(&[&[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[Key::Space], &[]]);
    let clock = Clock::fake(Duration::from_millis(150));
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.written().contains(&"Crashed into the wall with 0. Space plays again.".to_string()), "crashed");
    assert!(window.written().last().map(String::as_str) == Some("Score 0   Best 0   Arrows steer, Escape quits"), "started_again");
    assert!(best == 0, "no_score");
}

#[test]
fn escape_quits() {
    // y: covers play > open > going_on'1, steer > pressed
    let window = Window::fake(&[&[Key::Escape], &[], &[]]);
    let clock = Clock::fake(Duration::from_millis(150));
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.frames_shown() == 1, "one_frame");
    assert!(best == 0, "no_score");
}

#[test]
fn the_other_keys_turn_the_snake() {
    // y: covers turned > arrow.up'2, turned > arrow.down'1, turned > arrow.left'2, turned > arrow.right'1
    let heading_right = moving(Direction::Right);
    let heading_left = moving(Direction::Left);
    let heading_down = moving(Direction::Down);
    let heading_up = moving(Direction::Up);
    let up = turned(&heading_right, Key::W);
    let down = turned(&heading_left, Key::Down);
    let left = turned(&heading_down, Key::A);
    let right = turned(&heading_up, Key::Right);
    assert!(up == Direction::Up, "turned_up");
    assert!(down == Direction::Down, "turned_down");
    assert!(left == Direction::Left, "turned_left");
    assert!(right == Direction::Right, "turned_right");
}

#[test]
fn two_keys_in_a_frame_steer_in_turn() {
    // y: covers unless_reversing > turning, steer > pressed'many
    let game = moving(Direction::Right);
    steer(&mut game, &[Key::Up, Key::Left]);
    assert!(game.wanted == Direction::Up, "up: Expected up to stand: left, after up, is back the way the snake moved last.");
}

#[test]
fn the_snake_catches_up_with_the_time() {
    // y: covers catch_up > not_due'1, catch_up > not_due'2, catch_up > due'2gt, catch_up > due'many'2eq,
    // catch_up > due'many'2gt
    let clock = Clock::fake(Duration::from_secs(1));
    let random = Random::seeded(7);
    let start = clock.now();
    let early = moving(Direction::Right);
    let once = moving(Direction::Right);
    let twice = moving(Direction::Right);
    let over = moving(Direction::Right);
    over.standing = Outcome::Crashed;
    let not_yet = catch_up(&mut early, start, start + Duration::from_millis(100), &mut random);
    let one_step = catch_up(&mut once, start, start + Duration::from_millis(200), &mut random);
    let two_steps = catch_up(&mut twice, start, start + Duration::from_millis(300), &mut random);
    let none = catch_up(&mut over, start, start + Duration::from_millis(300), &mut random);
    assert!(not_yet == start && early.head == Cell { column: 10, row: 7 }, "early_stays");
    assert!(one_step == start + Duration::from_millis(150) && once.head == Cell { column: 11, row: 7 }, "once_moves");
    assert!(two_steps == start + Duration::from_millis(300) && twice.head == Cell { column: 12, row: 7 }, "twice_moves");
    assert!(none == start && over.head == Cell { column: 10, row: 7 }, "over_stays");
}

#[test]
fn a_snake_of_one_or_two_cells_is_drawn() {
    // y: covers banner > shown.playing, draw > no_tail, draw > part, fill_cell
    let window = Window::fake(&[]);
    let alone = moving(Direction::Right);
    let short = moving(Direction::Right);
    short.tail = vec![Cell { column: 9, row: 7 }];
    draw(&alone, 0, &mut window);
    draw(&short, 0, &mut window);
    assert!(window.written() == ["Score 0   Best 0   Arrows steer, Escape quits", "Score 0   Best 0   Arrows steer, Escape quits"], "both_written");
}

#[test]
fn a_window_closed_at_once_ends_play() {
    // y: covers play > closed_at_once'2
    let window = Window::fake(&[]);
    let clock = Clock::fake(Duration::from_millis(150));
    let random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(best == 0, "no_score");
}
