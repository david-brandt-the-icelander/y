//! Lowered from `y/src/snake.y`, the Y module `snake`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `snake::rules::advance`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm or its `if`.
//! `play` may suspend, because `window.next_frame()` may: Y has no `async`, and Rust's colouring makes it an `async fn`.

use y_runtime::{list, require, Clock, Forever, Random};
use y_window::{Colour, Key, Window};
use self::{drawing::*, rules::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the board, the rules, and how a game starts.
mod setup {
    use super::*;

    /// The board the snake moves on, in cells: the Y constant group `board`.
    mod board {
        const COLUMNS = 20;
        const ROWS = 15;
    }

    /// The rules every game is played by: the Y constant group `game_rules`.
    mod game_rules {
        use std::time::Duration;

        /// How long the snake takes to move one cell.
        const STEP_TIME = Duration::from_millis(150);
    }

    /// One cell of the board, counted from 0 at the top left.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Cell {
        column,
        row,
    }

    /// Which way the snake moves.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Direction {
        Up,
        Down,
        Left,
        Right,
    }

    /// Where a game stands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Playing,
        Crashed,
        Bitten,
        Filled,
    }

    /// One game.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Game {
        /// The cell the snake's head is in.
        head,
        /// The cells of the rest of the snake, the one nearest the head first.
        tail,
        /// The way the snake moved last.
        heading,
        /// The way it will move next, as the player last steered it.
        wanted,
        /// The cell the food is in.
        food,
        /// How much food the snake has eaten: the score.
        eaten,
        standing,
    }

    /// Starts a game: a snake three cells long in the left half of the board, moving right, and food in a free cell
    /// chosen at random.
    ///
    /// - `random`: the Y `injected` parameter.
    /// - Returns the game.
    fn new_game(random) {
        let fresh = Game {
            head: Cell { column: 6, row: 7 },
            tail: vec![Cell { column: 5, row: 7 }, Cell { column: 4, row: 7 }],
            heading: Direction::Right,
            wanted: Direction::Right,
            // until `place_food` chooses a free cell
            food: Cell { column: 6, row: 7 },
            eaten: 0,
            standing: Outcome::Playing,
        };
        place_food(&mut fresh, random);
        fresh
    }
}

/// The Y facet `rules`: how the player steers the snake, and how it moves.
mod rules {
    use super::*;

    /// Turns the snake the way an arrow key, or one of W, A, S and D, points. The snake cannot turn back on itself, so
    /// a key pointing back the way it moved last leaves it going as the player last steered it.
    ///
    /// - `game`: the game.
    /// - `pressed`: the key the player pressed.
    /// - Returns the way the snake will move next.
    fn turned(game, pressed) {
        match pressed {
            Key::Up | Key::W => unless_reversing(game, Direction::Up), // y: #arrow.up
            Key::Down | Key::S => unless_reversing(game, Direction::Down), // y: #arrow.down
            Key::Left | Key::A => unless_reversing(game, Direction::Left), // y: #arrow.left
            Key::Right | Key::D => unless_reversing(game, Direction::Right), // y: #arrow.right
            _ => game.wanted, // y: #arrow.other
        }
    }

    /// Moves the snake one cell the way the player last steered it. Moving into the wall or into the snake's own body
    /// ends the game. Moving onto the food makes the snake one cell longer, and puts new food in a free cell.
    ///
    /// - `game`: the game, which changes.
    /// - `random`: the Y `injected` parameter.
    fn advance(game, random) {
        require(game.standing == Outcome::Playing, "snake::advance#still_playing");
        game.heading = game.wanted;
        let ahead = ahead_of(&game.head, game.heading);
        if !on_board(&ahead) { // y: #moved.into_wall
            game.standing = Outcome::Crashed;
        } else if list::without_last(&game.tail).contains(&ahead) { // y: #moved.into_itself
            game.standing = Outcome::Bitten;
        } else if ahead == game.food { // y: #moved.onto_food
            game.tail.insert(0, game.head);
            game.head = ahead;
            game.eaten += 1;
            place_food(game, random);
        } else { // y: #moved.on
            game.tail.insert(0, game.head);
            game.head = ahead;
            game.tail.pop();
        }
    }
}

/// The way the snake will move next, unless the player is steering it back the way it moved last.
///
/// - `game`: the game.
/// - `toward`: the way the player steered.
/// - Returns `toward`, or the way the player steered before when `toward` turns the snake back on itself.
fn unless_reversing(game, toward) {
    if toward == opposite(game.heading) { // y: #reversing
        game.wanted
    } else { // y: #turning
        toward
    }
}

/// The way back.
///
/// - `heading`: a way to move.
/// - Returns the opposite way.
fn opposite(heading) {
    match heading {
        Direction::Up => Direction::Down, // y: #going.up
        Direction::Down => Direction::Up, // y: #going.down
        Direction::Left => Direction::Right, // y: #going.left
        Direction::Right => Direction::Left, // y: #going.right
    }
}

/// The next cell one way. It may be off the board.
///
/// - `here`: the cell to move from.
/// - `heading`: the way to move.
/// - Returns the cell next to `here` that way.
fn ahead_of(here, heading) {
    match heading {
        Direction::Up => Cell { column: here.column, row: here.row - 1 }, // y: #going.up
        Direction::Down => Cell { column: here.column, row: here.row + 1 }, // y: #going.down
        Direction::Left => Cell { column: here.column - 1, row: here.row }, // y: #going.left
        Direction::Right => Cell { column: here.column + 1, row: here.row }, // y: #going.right
    }
}

/// Whether a cell is on the board.
///
/// - `place`: the cell.
/// - Returns whether it is.
fn on_board(place) {
    place.column >= 0 && place.column < board::COLUMNS && place.row >= 0 && place.row < board::ROWS
}

/// The cells the snake is not in, row by row from the top left.
///
/// - `game`: the game.
/// - Returns the cells.
fn free_cells(game) {
    let found = Vec::new();
    for row in 0..board::ROWS {
        for column in 0..board::COLUMNS {
            let place = Cell { column, row };
            if place == game.head || game.tail.contains(&place) { // y: #taken
                continue;
            } else { // y: #vacant
                found.push(place);
            }
        }
    }
    found
}

/// Puts the food in a free cell chosen at random. When there is none, the snake fills the board, and the game ends.
///
/// - `game`: the game, which changes.
/// - `random`: the Y `injected` parameter.
fn place_food(game, random) {
    let free = free_cells(game);
    if free.is_empty() { // y: #full
        game.standing = Outcome::Filled;
    } else { // y: #room
        game.food = free[random.index_below(free.len())];
    }
}

/// The Y facet `drawing`: what the player sees of a game.
mod drawing {
    use super::*;

    /// The sizes of what is drawn, in pixels: the Y constant group `layout`.
    mod layout {
        use super::*;

        /// The side of one cell.
        const CELL_SIZE = 32;
        /// The height of the bar beneath the board, where the score is written.
        const BAR_HEIGHT = 40;
        const WIDTH = board::COLUMNS * CELL_SIZE;
        const HEIGHT = board::ROWS * CELL_SIZE + BAR_HEIGHT;
    }

    /// The colours of what is drawn: the Y constant group `palette`.
    mod palette {
        use super::*;

        const BACKGROUND = Colour::rgb(24, 26, 33);
        const BOARD = Colour::rgb(36, 42, 52);
        const HEAD = Colour::rgb(140, 230, 110);
        const BODY = Colour::rgb(70, 170, 80);
        const FOOD = Colour::rgb(235, 85, 70);
        const TEXT = Colour::rgb(230, 230, 230);
    }

    /// The line written beneath the board: the score, and what the player can do.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - Returns the text.
    fn banner(game, highest) {
        match game.standing {
            Outcome::Playing => format!("Score {}   Best {highest}   Arrows steer, Escape quits", game.eaten), // y: #shown.playing
            Outcome::Crashed => format!("Crashed into the wall with {}. Space plays again.", game.eaten), // y: #shown.crashed
            Outcome::Bitten => format!("Bit itself with {}. Space plays again.", game.eaten), // y: #shown.bitten
            Outcome::Filled => format!("Filled the board with {}! Space plays again.", game.eaten), // y: #shown.filled
        }
    }

    /// Draws a game: the board, the food, the snake, and the banner beneath.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - `window`: the Y `injected` parameter.
    fn draw(game, highest, window) {
        let board_height = board::ROWS * layout::CELL_SIZE;
        window.clear(palette::BACKGROUND);
        window.fill(0, 0, layout::WIDTH, board_height, palette::BOARD);
        fill_cell(&game.food, palette::FOOD, window);
        for part in &game.tail {
            fill_cell(part, palette::BODY, window);
        }
        fill_cell(&game.head, palette::HEAD, window);
        let line = banner(game, highest);
        window.write(&line, 12, layout::HEIGHT - 14, 24, palette::TEXT);
    }
}

/// Fills one cell, leaving a pixel's gap on each side so that the cells stand apart.
///
/// - `place`: the cell.
/// - `shade`: its colour.
/// - `window`: the Y `injected` parameter.
fn fill_cell(place, shade, window) {
    let x = place.column * layout::CELL_SIZE + 1;
    let y = place.row * layout::CELL_SIZE + 1;
    window.fill(x, y, layout::CELL_SIZE - 2, layout::CELL_SIZE - 2, shade);
}

/// The Y facet `window_play`: playing in a window, one game after another, until the window is closed.
mod window_play {
    use super::*;

    /// Plays Snake in a window: one game after another, until the player closes the window or presses Escape. When a
    /// game ends, Space starts the next.
    ///
    /// - `window`, `clock`, `random`, `forever`: the Y `injected` parameters.
    /// - Returns the best score of the games played.
    async fn play(window, clock, random, forever) {
        let game = new_game(random);
        let highest = 0;
        let last_step = clock.now();
        let quitting = false;
        while !quitting && !window.close_requested() { // y: #open
            let keys = window.keys_pressed();
            let now = clock.now();
            quitting = keys.contains(&Key::Escape);
            for pressed in &keys {
                game.wanted = turned(&game, *pressed);
            }
            if game.standing != Outcome::Playing && keys.contains(&Key::Space) { // y: #again
                game = new_game(random);
                last_step = now;
            } else { // y: #going_on
                while game.standing == Outcome::Playing && now - last_step >= game_rules::STEP_TIME { // y: #due
                    advance(&mut game, random);
                    last_step += game_rules::STEP_TIME;
                }
                // y: #caught_up, the `else` of `while due`: the loop has no `break`, so it runs when the loop ends
                highest = highest.max(game.eaten);
            }
            draw(&game, highest, window);
            window.next_frame(forever).await; // y: forever because "a hidden window shows no frames, and the game waits until it is shown"
        }
        // y: #closed, the `else` of `while open`: the loop has no `break`, so it runs when the loop ends
        highest
    }
}
