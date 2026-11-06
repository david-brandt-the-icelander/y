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
fn moving(heading: Direction) -> Game {
    Game {
        head: Cell { column: 10, row: 7 },
        tail: List::new(),
        heading,
        wanted: heading,
        food: Cell { column: 0, row: 0 },
        eaten: 0,
        standing: Outcome::Playing,
    }
}

#[test]
fn a_new_game_puts_the_food_in_a_free_cell() {
    // y: covers new_game
    let mut random = Random::seeded(7);
    let started = new_game(&mut random);
    assert!(started.tail.count() == 2, "three_long");
    assert!(started.heading == Direction::Right, "moving_right");
    assert!(started.food != started.head && !started.tail.contains(&started.food), "food_off_the_snake");
}

#[test]
fn the_up_arrow_turns_the_snake_up() {
    // y: covers turned > arrow.up'1
    let game = moving(Direction::Right);
    let wanted = turned(&game, Key::Up);
    assert!(wanted == Direction::Up, "turned_up");
}

#[test]
fn w_turns_the_snake_up() {
    // y: covers turned > arrow.up'2
    let game = moving(Direction::Right);
    let wanted = turned(&game, Key::W);
    assert!(wanted == Direction::Up, "turned_up");
}

#[test]
fn the_down_arrow_turns_the_snake_down() {
    // y: covers turned > arrow.down'1
    let game = moving(Direction::Right);
    let wanted = turned(&game, Key::Down);
    assert!(wanted == Direction::Down, "turned_down");
}

#[test]
fn s_turns_the_snake_down() {
    // y: covers turned > arrow.down'2
    let game = moving(Direction::Right);
    let wanted = turned(&game, Key::S);
    assert!(wanted == Direction::Down, "turned_down");
}

#[test]
fn the_left_arrow_turns_the_snake_left() {
    // y: covers turned > arrow.left'1
    let game = moving(Direction::Up);
    let wanted = turned(&game, Key::Left);
    assert!(wanted == Direction::Left, "turned_left");
}

#[test]
fn a_turns_the_snake_left() {
    // y: covers turned > arrow.left'2
    let game = moving(Direction::Up);
    let wanted = turned(&game, Key::A);
    assert!(wanted == Direction::Left, "turned_left");
}

#[test]
fn the_right_arrow_turns_the_snake_right() {
    // y: covers turned > arrow.right'1
    let game = moving(Direction::Up);
    let wanted = turned(&game, Key::Right);
    assert!(wanted == Direction::Right, "turned_right");
}

#[test]
fn d_turns_the_snake_right() {
    // y: covers turned > arrow.right'2
    let game = moving(Direction::Up);
    let wanted = turned(&game, Key::D);
    assert!(wanted == Direction::Right, "turned_right");
}

#[test]
fn a_key_that_is_not_an_arrow_does_not_turn_the_snake() {
    // y: covers turned > arrow.other
    let mut game = moving(Direction::Right);
    game.wanted = Direction::Up;
    let wanted = turned(&game, Key::Space);
    assert!(wanted == Direction::Up, "unchanged");
}

#[test]
fn the_tail_follows_the_head() {
    // y: covers advance > moved.on
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }, Cell { column: 8, row: 7 }];
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.head == Cell { column: 11, row: 7 }, "moved");
    assert!(game.tail == y_collections::list![Cell { column: 10, row: 7 }, Cell { column: 9, row: 7 }], "followed");
}

#[test]
fn the_snake_may_follow_its_own_tail() {
    // y: covers advance > moved.on
    let mut game = moving(Direction::Left);
    game.head = Cell { column: 5, row: 5 };
    game.tail = y_collections::list![Cell { column: 6, row: 5 }, Cell { column: 6, row: 6 }, Cell { column: 5, row: 6 }];
    game.wanted = Direction::Down;
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Playing, "still_playing");
    assert!(game.head == Cell { column: 5, row: 6 }, "moved");
}

#[test]
fn the_wall_ends_the_game() {
    // y: covers advance > moved.into_wall
    let mut game = moving(Direction::Right);
    game.head = Cell { column: 19, row: 7 };
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Crashed, "crashed");
    assert!(game.head == Cell { column: 19, row: 7 }, "not_moved");
}

#[test]
fn the_snake_cannot_bite_itself() {
    // y: covers advance > moved.into_itself
    let mut game = moving(Direction::Left);
    game.head = Cell { column: 5, row: 5 };
    game.tail = y_collections::list![Cell { column: 6, row: 5 }, Cell { column: 6, row: 6 }, Cell { column: 5, row: 6 }, Cell { column: 4, row: 6 }];
    game.wanted = Direction::Down;
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.standing == Outcome::Bitten, "bitten");
    assert!(game.head == Cell { column: 5, row: 5 }, "not_moved");
}

#[test]
fn eating_grows_the_snake() {
    // y: covers advance > moved.onto_food
    let mut game = moving(Direction::Left);
    game.food = Cell { column: 9, row: 7 };
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.tail == y_collections::list![Cell { column: 10, row: 7 }], "grown");
    assert!(game.eaten == 1, "scored");
    assert!(game.food != game.head && !game.tail.contains(&game.food), "new_food");
}

#[test]
fn eating_the_last_food_fills_the_board() {
    // y: covers advance > moved.onto_food
    let corner = Cell { column: 0, row: 0 };
    let mut game = moving(Direction::Left);
    game.head = Cell { column: 1, row: 0 };
    game.tail = y_collections::list![corner];
    let rest = free_cells(&game);
    game.tail = rest;
    game.food = corner;
    let mut random = Random::seeded(7);
    advance(&mut game, &mut random);
    assert!(game.eaten == 1, "scored");
    assert!(game.standing == Outcome::Filled, "filled");
}

#[test]
fn steering_back_the_way_the_snake_moved_keeps_the_way_wanted() {
    // y: covers unless_reversing > reversing
    let mut game = moving(Direction::Right);
    game.wanted = Direction::Up;
    let wanted = unless_reversing(&game, Direction::Left);
    assert!(wanted == Direction::Up, "still_up");
}

#[test]
fn steering_another_way_turns_the_snake() {
    // y: covers unless_reversing > turning
    let game = moving(Direction::Right);
    let wanted = unless_reversing(&game, Direction::Up);
    assert!(wanted == Direction::Up, "turned_up");
}

#[test]
fn the_opposite_of_up_is_down() {
    // y: covers opposite > going.up
    let back = opposite(Direction::Up);
    assert!(back == Direction::Down, "down");
}

#[test]
fn the_opposite_of_down_is_up() {
    // y: covers opposite > going.down
    let back = opposite(Direction::Down);
    assert!(back == Direction::Up, "up");
}

#[test]
fn the_opposite_of_left_is_right() {
    // y: covers opposite > going.left
    let back = opposite(Direction::Left);
    assert!(back == Direction::Right, "right");
}

#[test]
fn the_opposite_of_right_is_left() {
    // y: covers opposite > going.right
    let back = opposite(Direction::Right);
    assert!(back == Direction::Left, "left");
}

#[test]
fn the_cell_ahead_going_up() {
    // y: covers ahead_of > going.up
    let ahead = ahead_of(&Cell { column: 10, row: 7 }, Direction::Up);
    assert!(ahead == Cell { column: 10, row: 6 }, "next_up");
}

#[test]
fn the_cell_ahead_going_down() {
    // y: covers ahead_of > going.down
    let ahead = ahead_of(&Cell { column: 10, row: 7 }, Direction::Down);
    assert!(ahead == Cell { column: 10, row: 8 }, "next_down");
}

#[test]
fn the_cell_ahead_going_left() {
    // y: covers ahead_of > going.left
    let ahead = ahead_of(&Cell { column: 10, row: 7 }, Direction::Left);
    assert!(ahead == Cell { column: 9, row: 7 }, "next_left");
}

#[test]
fn the_cell_ahead_going_right() {
    // y: covers ahead_of > going.right
    let ahead = ahead_of(&Cell { column: 10, row: 7 }, Direction::Right);
    assert!(ahead == Cell { column: 11, row: 7 }, "next_right");
}

#[test]
fn the_far_corner_is_on_the_board() {
    // y: covers on_board
    let inside = on_board(&Cell { column: 19, row: 14 });
    assert!(inside, "on");
}

#[test]
fn a_cell_past_the_edge_is_off_the_board() {
    // y: covers on_board
    let inside = on_board(&Cell { column: 20, row: 7 });
    assert!(!inside, "off");
}

#[test]
fn cells_the_snake_is_not_in_are_free() {
    // y: covers free_cells > row'many > column'many > vacant
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }];
    let free = free_cells(&game);
    assert!(free.count() == 298, "all_the_others");
    assert!(free.front() == Some(&Cell { column: 0, row: 0 }), "from_the_top_left");
}

#[test]
fn the_cell_of_the_head_is_not_free() {
    // y: covers free_cells > row'many > column'many > taken'1
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }];
    let free = free_cells(&game);
    assert!(!free.contains(&game.head), "head_taken");
}

#[test]
fn the_cells_of_the_tail_are_not_free() {
    // y: covers free_cells > row'many > column'many > taken'2
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }];
    let free = free_cells(&game);
    assert!(!free.contains(&Cell { column: 9, row: 7 }), "tail_taken");
}

#[test]
fn food_with_no_free_cell_fills_the_board() {
    // y: covers place_food > full
    let mut game = moving(Direction::Left);
    game.head = Cell { column: 0, row: 0 };
    game.tail = free_cells(&game);
    let mut random = Random::seeded(7);
    place_food(&mut game, &mut random);
    assert!(game.standing == Outcome::Filled, "filled");
}

#[test]
fn food_goes_in_a_free_cell() {
    // y: covers place_food > room
    let mut game = moving(Direction::Left);
    game.head = Cell { column: 0, row: 0 };
    game.tail = free_cells(&game);
    game.tail.take_back();
    let mut random = Random::seeded(7);
    place_food(&mut game, &mut random);
    assert!(game.food == Cell { column: 19, row: 14 }, "in_the_free_cell");
    assert!(game.standing == Outcome::Playing, "still_playing");
}

#[test]
fn the_banner_of_a_game_in_play_shows_the_score_and_the_best() {
    // y: covers banner > shown.playing
    let mut game = moving(Direction::Right);
    game.eaten = 3;
    let line = banner(&game, 5);
    assert!(line == "Score 3   Best 5   Arrows steer, Escape quits", "told");
}

#[test]
fn the_banner_tells_of_a_crash_into_the_wall() {
    // y: covers banner > shown.crashed
    let mut game = moving(Direction::Right);
    game.standing = Outcome::Crashed;
    game.eaten = 2;
    let line = banner(&game, 5);
    assert!(line == "Crashed into the wall with 2. Space plays again.", "told");
}

#[test]
fn the_banner_tells_of_a_bite() {
    // y: covers banner > shown.bitten
    let mut game = moving(Direction::Right);
    game.standing = Outcome::Bitten;
    game.eaten = 4;
    let line = banner(&game, 5);
    assert!(line == "Bit itself with 4. Space plays again.", "told");
}

#[test]
fn the_banner_tells_of_a_filled_board() {
    // y: covers banner > shown.filled
    let mut game = moving(Direction::Right);
    game.standing = Outcome::Filled;
    game.eaten = 298;
    let line = banner(&game, 5);
    assert!(line == "Filled the board with 298! Space plays again.", "told");
}

#[test]
fn a_snake_with_no_tail_is_drawn() {
    // y: covers draw > no_tail
    let mut window = Window::fake(&[]);
    let game = moving(Direction::Right);
    draw(&game, 2, &mut window);
    assert!(window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"], "banner_written");
}

#[test]
fn a_snake_with_a_tail_of_one_cell_is_drawn() {
    // y: covers draw > part
    let mut window = Window::fake(&[]);
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }];
    draw(&game, 2, &mut window);
    assert!(window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"], "banner_written");
}

#[test]
fn a_snake_with_a_longer_tail_is_drawn() {
    // y: covers draw > part'many
    let mut window = Window::fake(&[]);
    let mut game = moving(Direction::Right);
    game.tail = y_collections::list![Cell { column: 9, row: 7 }, Cell { column: 8, row: 7 }];
    draw(&game, 2, &mut window);
    assert!(window.written() == ["Score 0   Best 2   Arrows steer, Escape quits"], "banner_written");
}

#[test]
fn filling_a_cell_fills_one_square() {
    // y: covers fill_cell
    let mut window = Window::fake(&[]);
    fill_cell(&Cell { column: 3, row: 4 }, palette::FOOD, &mut window);
    assert!(window.fills() == 1, "one_square");
    assert!(window.written().is_empty(), "nothing_written");
}

#[test]
fn a_window_closed_at_once_ends_play() {
    // y: covers play > closed_at_once'2
    let mut window = Window::fake(&[]);
    let mut clock = Clock::fake(Duration::from_millis(150));
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.frames_shown() == 0, "no_frames");
    assert!(best == 0, "no_score");
}

#[test]
fn escape_quits() {
    // y: covers play > open > going_on'1
    let mut window = Window::fake(&[&[Key::Escape], &[], &[]]);
    let mut clock = Clock::fake(Duration::from_millis(150));
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.frames_shown() == 1, "one_frame");
    assert!(best == 0, "no_score");
}

#[test]
fn the_game_plays_until_the_window_closes() {
    // y: covers play > open'many > going_on'1
    let mut window = Window::fake(&[&[], &[], &[]]);
    let mut clock = Clock::fake(Duration::from_millis(150));
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.frames_shown() == 3, "three_frames");
    assert!(window.written().last().map(String::as_str) == Some("Score 0   Best 0   Arrows steer, Escape quits"), "drawn");
    assert!(best == 0, "no_score");
}

#[test]
fn space_starts_the_next_game() {
    // y: covers play > open'many > again
    let mut window = Window::fake(&[&[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[Key::Space], &[]]);
    let mut clock = Clock::fake(Duration::from_millis(150));
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.written().contains(&"Crashed into the wall with 0. Space plays again.".to_string()), "crashed");
    assert!(window.written().last().map(String::as_str) == Some("Score 0   Best 0   Arrows steer, Escape quits"), "started_again");
    assert!(best == 0, "no_score");
}

#[test]
fn a_finished_game_waits_for_space() {
    // y: covers play > open'many > going_on'2
    let mut window = Window::fake(&[&[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[], &[]]);
    let mut clock = Clock::fake(Duration::from_millis(150));
    let mut random = Random::seeded(7);
    let forever = Forever::fake();
    let best = block_on(play(&mut window, &mut clock, &mut random, &forever));
    assert!(window.written().last().map(String::as_str) == Some("Crashed into the wall with 0. Space plays again."), "still_over");
    assert!(best == 0, "no_score");
}

#[test]
fn no_key_leaves_the_way_wanted() {
    // y: covers steer > no_keys
    let mut game = moving(Direction::Right);
    steer(&mut game, &List::new());
    assert!(game.wanted == Direction::Right, "unchanged");
}

#[test]
fn a_key_pressed_steers_the_snake() {
    // y: covers steer > pressed
    let mut game = moving(Direction::Right);
    steer(&mut game, &y_collections::list![Key::Up]);
    assert!(game.wanted == Direction::Up, "up");
}

#[test]
fn two_keys_in_a_frame_steer_in_turn() {
    // y: covers steer > pressed'many
    let mut game = moving(Direction::Right);
    steer(&mut game, &y_collections::list![Key::Up, Key::Left]);
    assert!(game.wanted == Direction::Up, "up: Expected up to stand: left, after up, is back the way the snake moved last.");
}

#[test]
fn a_game_that_is_over_does_not_step() {
    // y: covers catch_up > not_due'1
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    game.standing = Outcome::Crashed;
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(300), &mut random);
    assert!(stepped == start, "last_step");
    assert!(game.head == Cell { column: 10, row: 7 }, "head");
}

#[test]
fn the_snake_waits_for_a_whole_step_time() {
    // y: covers catch_up > not_due'2
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(100), &mut random);
    assert!(stepped == start, "last_step");
    assert!(game.head == Cell { column: 10, row: 7 }, "head");
}

#[test]
fn the_snake_steps_once_a_step_time_exactly_after() {
    // y: covers catch_up > due'2eq
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(150), &mut random);
    assert!(stepped == start + Duration::from_millis(150), "last_step");
    assert!(game.head == Cell { column: 11, row: 7 }, "head");
}

#[test]
fn the_snake_steps_once_between_one_and_two_step_times() {
    // y: covers catch_up > due'2gt
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(200), &mut random);
    assert!(stepped == start + Duration::from_millis(150), "last_step");
    assert!(game.head == Cell { column: 11, row: 7 }, "head");
}

#[test]
fn the_snake_steps_twice_two_step_times_exactly_after() {
    // y: covers catch_up > due'many'2eq
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(300), &mut random);
    assert!(stepped == start + Duration::from_millis(300), "last_step");
    assert!(game.head == Cell { column: 12, row: 7 }, "head");
}

#[test]
fn the_snake_steps_twice_between_two_and_three_step_times() {
    // y: covers catch_up > due'many'2gt
    let mut clock = Clock::fake(Duration::from_secs(1));
    let mut random = Random::seeded(7);
    let start = clock.now();
    let mut game = moving(Direction::Right);
    let stepped = catch_up(&mut game, start, start + Duration::from_millis(350), &mut random);
    assert!(stepped == start + Duration::from_millis(300), "last_step");
    assert!(game.head == Cell { column: 12, row: 7 }, "head");
}
