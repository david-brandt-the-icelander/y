//! Lowered from `y/src/memory.y`, the Y module `memory`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `memory::rules::turn_over`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm or its `if`.
//! `new_game` may suspend, because `site.get()` may, and `play` may, because it calls `new_game` and
//! `window.next_frame()`: Y has no `async`, and Rust's colouring makes them `async fn`s.

use std::time::{Duration, Instant};
use y_http::{Http, HttpError};
use y_runtime::{ensure, list, require, time, Clock, Deadline, Expired, Forever, Random, Within};
use y_window::{Colour, Key, Picture, PictureError, Point, Window};
use self::{drawing::*, rules::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the board, the rules, the pictures, and how a game starts.
pub mod setup {
    use super::*;

    /// The board: the Y constant group `board`.
    pub mod board {
        /// How many cards are laid out across the board, and how many down it.
        pub const COLUMNS: i32 = 5;
        pub const ROWS: i32 = 4;
        /// How many cards there are, numbered from 1 row by row from the top left.
        pub const CARDS: u32 = 20;
        /// How many pairs of cards there are: one for each picture.
        pub const PAIRS: u32 = 10;
    }

    /// The rules every game is played by, and how it is scored: the Y constant group `game_rules`.
    pub mod game_rules {
        use std::time::Duration;

        /// How many times the player may turn over two cards that do not match before the game is lost.
        pub const LIVES: u32 = 10;
        /// How long two cards that do not match stay face up.
        pub const SHOW_TIME: Duration = Duration::from_secs(1);
        /// How long the pictures may take to fetch.
        pub const FETCH_TIME: Duration = Duration::from_secs(20);
        /// What each pair found scores.
        pub const POINTS_PER_PAIR: u32 = 100;
        /// What each life left scores, when every pair is found.
        pub const POINTS_PER_LIFE: u32 = 50;
        /// What each whole second under the par time scores, when every pair is found.
        pub const POINTS_PER_SECOND: u32 = 10;
        /// The time to beat.
        pub const PAR_TIME: Duration = Duration::from_secs(120);
    }

    /// The pictures the cards may show: the Y constant group `faces`.
    pub mod faces {
        /// The OpenMoji codes of twenty animals' faces, from which each game chooses its pictures.
        pub const CODES: [&str; 20] = [
            "1F436", "1F431", "1F42D", "1F439", "1F430", "1F98A", "1F43B", "1F43C", "1F428", "1F42F", "1F981", "1F42E",
            "1F437", "1F438", "1F435", "1F414", "1F427", "1F426", "1F989", "1F984",
        ];
    }

    /// OpenMoji, the open-source emoji project, whose pictures the cards show: the Y service `openmoji`. Its pictures
    /// are licensed CC BY-SA 4.0, and the game credits them.
    pub mod openmoji {
        pub const ADDRESS: &str = "https://openmoji.org";
    }

    /// Which side of a card is up.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Side {
        FaceDown,
        FaceUp,
        Matched,
    }

    /// Where a game stands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Outcome {
        Playing,
        Won,
        Lost,
    }

    /// One game.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Game {
        /// For each card, in the order they are numbered, the number of the picture it shows.
        pub(super) pairs: Vec<u32>,
        /// For each card, in the same order, which side is up.
        pub(super) sides: Vec<Side>,
        /// The pictures, numbered from 1.
        pub(super) pictures: Vec<Picture>,
        /// The cards turned face up this turn, and not yet matched: none, one or two.
        pub(super) turned: Vec<u32>,
        /// When the second card of the turn was turned, or when the game started.
        pub(super) turned_at: Instant,
        /// How many pairs have been found.
        pub(super) matches: u32,
        /// How many lives are left.
        pub(super) lives: u32,
        /// When the game started.
        pub(super) started: Instant,
        /// How long the game has been played, which stops when it ends.
        pub(super) elapsed: Duration,
    }

    /// Why a game could not be set up.
    #[derive(Clone, Debug, PartialEq)]
    pub enum SetupError {
        Unreachable { error: HttpError },
        TimedOut { deadline: Expired },
        NotAPicture { error: PictureError },
    }

    /// Y's `or fail` inside `within` converts the call's own failure, or the expiry, through `SetupError`'s case that
    /// carries it (Y § 3.4, § 3.8).
    impl From<Within<HttpError>> for SetupError {
        fn from(failed: Within<HttpError>) -> SetupError {
            match failed {
                Within::Failed(error) => SetupError::Unreachable { error },
                Within::Expired(deadline) => SetupError::TimedOut { deadline },
            }
        }
    }

    /// Y's `or fail` on a `PictureError` converts through `SetupError`'s one case that carries it (Y § 3.4).
    impl From<PictureError> for SetupError {
        fn from(error: PictureError) -> SetupError {
            SetupError::NotAPicture { error }
        }
    }

    /// Starts a game: chooses the pictures at random, fetches them from OpenMoji, and deals two cards of each, face
    /// down, in an order drawn at random.
    ///
    /// - `random`, `clock`, `site`: the Y `injected` parameters.
    /// - Returns the game.
    /// - Fails `Unreachable`: OpenMoji did not give a picture.
    /// - Fails `TimedOut`: the pictures took longer than the fetch time to come.
    /// - Fails `NotAPicture`: what OpenMoji gave is not a picture.
    pub async fn new_game(random: &mut Random, clock: &mut Clock, site: &Http) -> Result<Game, SetupError> {
        let codes = random.chosen(&faces::CODES, board::PAIRS);
        let mut pictures = Vec::new();
        {
            // y: within game_rules.fetch_time or fail
            let deadline = Deadline::after(game_rules::FETCH_TIME);
            for code in codes { // y: #code
                let drawing = site.get(&format!("/data/color/svg/{code}.svg"), &deadline).await?;
                let made = Picture::from_svg(&drawing, layout::PICTURE_SIZE)?;
                pictures.push(made);
            }
        }
        let mut dealt = Vec::new();
        for number in 1..=board::PAIRS { // y: #number
            dealt.push(number);
            dealt.push(number);
        }
        let now = clock.now();
        let started = Game {
            pairs: random.shuffled(&dealt),
            sides: list::repeated(Side::FaceDown, board::CARDS),
            pictures,
            turned: Vec::new(),
            turned_at: now,
            matches: 0,
            lives: game_rules::LIVES,
            started: now,
            elapsed: Duration::ZERO,
        };
        ensure(standing(&started) == Outcome::Playing, "memory::new_game#playing");
        ensure(started.turned.is_empty(), "memory::new_game#nothing_turned");
        Ok(started)
    }
}

/// The Y facet `rules`: turning cards over, and how a game ends and is scored.
pub mod rules {
    use super::*;

    /// The card the player may turn over by clicking a point, if there is one.
    ///
    /// - `game`: the game.
    /// - `place`: the point clicked.
    /// - Returns the card's number, or none when the point is not on a card, the card is face up, or two cards are
    ///   already face up.
    pub fn choosable(game: &Game, place: Point) -> Option<u32> {
        let Some(number) = card_at(place) else { // y: #judged.off_cards, which narrows `number` to a `uint32` for the rest
            return None;
        };
        if list::count(&game.turned) == 2 { // y: #judged.waiting
            None
        } else if list::at_number(&game.sides, number) != Side::FaceDown { // y: #judged.showing
            None
        } else { // y: #judged.free
            Some(number)
        }
    }

    /// Turns a card face up. The second card of a turn either matches the first, and the pair stays face up, or it
    /// does not, and costs a life.
    ///
    /// - `game`: the game, which changes.
    /// - `number`: the card's number.
    /// - `now`: the time now.
    pub fn turn_over(game: &mut Game, number: u32, now: Instant) {
        require(list::at_number(&game.sides, number) == Side::FaceDown && list::count(&game.turned) < 2, "memory::turn_over#may_turn");
        if game.turned.is_empty() { // y: #turn.first
            list::set_number(&mut game.sides, number, Side::FaceUp);
            game.turned.push(number);
        } else if list::at_number(&game.pairs, game.turned[0]) == list::at_number(&game.pairs, number) { // y: #turn.matching
            list::set_number(&mut game.sides, game.turned[0], Side::Matched);
            list::set_number(&mut game.sides, number, Side::Matched);
            game.turned = Vec::new();
            game.matches += 1;
        } else { // y: #turn.different
            list::set_number(&mut game.sides, number, Side::FaceUp);
            game.turned.push(number);
            game.turned_at = now;
            game.lives -= 1;
        }
    }

    /// Turns two cards that did not match face down again.
    ///
    /// - `game`: the game, which changes.
    pub fn turn_back(game: &mut Game) {
        require(list::count(&game.turned) == 2, "memory::turn_back#two_turned");
        for number in &game.turned { // y: #number
            list::set_number(&mut game.sides, *number, Side::FaceDown);
        }
        game.turned = Vec::new();
    }

    /// Where a game stands: won when every pair is found, lost when no life is left, and otherwise still being played.
    ///
    /// - `game`: the game.
    /// - Returns where it stands.
    pub fn standing(game: &Game) -> Outcome {
        if game.matches == board::PAIRS { // y: #judged.won
            Outcome::Won
        } else if game.lives == 0 { // y: #judged.lost
            Outcome::Lost
        } else { // y: #judged.playing
            Outcome::Playing
        }
    }

    /// The score of a game: points for each pair found and, when every pair is found, for each life left and each
    /// whole second under the par time.
    ///
    /// - `game`: the game.
    /// - Returns the score.
    pub fn score(game: &Game) -> u32 {
        let found = game.matches * game_rules::POINTS_PER_PAIR;
        if standing(game) == Outcome::Won { // y: #won
            found + game.lives * game_rules::POINTS_PER_LIFE + time_bonus(game.elapsed)
        } else { // y: #unfinished
            found
        }
    }
}

/// The points for finishing a game under the par time.
///
/// - `elapsed`: how long the game took.
/// - Returns the points for each whole second under the par time, or none when it took longer.
fn time_bonus(elapsed: Duration) -> u32 {
    if elapsed < game_rules::PAR_TIME { // y: #quick
        time::whole_seconds(game_rules::PAR_TIME - elapsed) * game_rules::POINTS_PER_SECOND
    } else { // y: #slow
        0
    }
}

/// The card under a point, if there is one.
///
/// - `place`: the point.
/// - Returns the card's number, or none when the point is between the cards or off the board.
fn card_at(place: Point) -> Option<u32> {
    let mut number = 0;
    for row in 0..board::ROWS { // y: #row
        for column in 0..board::COLUMNS { // y: #column
            number += 1;
            let corner = corner_of(row, column);
            let across = place.x >= corner.x && place.x < corner.x + layout::CARD_SIZE;
            let down = place.y >= corner.y && place.y < corner.y + layout::CARD_SIZE;
            if across && down { // y: #inside
                return Some(number);
            } else { // y: #outside
                continue;
            }
        }
    }
    None
}

/// The top left corner of the card in a row and a column, each counted from 0.
///
/// - `row`: the row.
/// - `column`: the column.
/// - Returns the corner.
fn corner_of(row: i32, column: i32) -> Point {
    Point { x: layout::GAP + column * layout::STEP, y: layout::GAP + row * layout::STEP }
}

/// The Y facet `drawing`: what the player sees of a game.
pub mod drawing {
    use super::*;

    /// The sizes of what is drawn, in pixels: the Y constant group `layout`.
    pub mod layout {
        use super::board;

        /// The side of a card.
        pub const CARD_SIZE: i32 = 112;
        /// The space between two cards, and around them.
        pub const GAP: i32 = 12;
        /// From one card to the next.
        pub const STEP: i32 = CARD_SIZE + GAP;
        /// How far a picture is drawn in from its card's edge.
        pub const PICTURE_INSET: i32 = 8;
        /// The side of a picture: the card's, less the inset on both sides.
        pub const PICTURE_SIZE: u16 = 96;
        /// The height of the bar beneath the board, where the score is written.
        pub const BAR_HEIGHT: i32 = 84;
        /// The height of the text.
        pub const TEXT_SIZE: u32 = 20;
        pub const WIDTH: i32 = GAP + board::COLUMNS * STEP;
        pub const HEIGHT: i32 = GAP + board::ROWS * STEP + BAR_HEIGHT;
    }

    /// The colours of what is drawn: the Y constant group `palette`.
    pub mod palette {
        use super::Colour;

        pub const BACKGROUND: Colour = Colour::rgb(24, 26, 33);
        pub const BACK: Colour = Colour::rgb(70, 90, 140);
        pub const FACE: Colour = Colour::rgb(235, 235, 225);
        pub const MATCHED: Colour = Colour::rgb(170, 215, 160);
        pub const TEXT: Colour = Colour::rgb(230, 230, 230);
        pub const CREDIT: Colour = Colour::rgb(140, 140, 150);
    }

    /// The lines written beneath the board: how the game stands, and what the player can do.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - Returns the two lines.
    pub fn banner(game: &Game, highest: u32) -> Vec<String> {
        let seconds = time::whole_seconds(game.elapsed);
        match standing(game) {
            Outcome::Playing => vec![ // y: #shown.playing
                format!("Pairs {} of {}   Lives {}   Time {seconds}s   Best {highest}", game.matches, board::PAIRS, game.lives),
                "Click two cards to turn them over. Escape quits.".to_string(),
            ],
            Outcome::Won => vec![ // y: #shown.won
                format!("All pairs in {seconds}s! Score {}   Best {highest}", score(game)),
                "Space plays again. Escape quits.".to_string(),
            ],
            Outcome::Lost => vec![ // y: #shown.lost
                format!("Out of lives with {} pairs. Score {}   Best {highest}", game.matches, score(game)),
                "Space plays again. Escape quits.".to_string(),
            ],
        }
    }

    /// Draws a game: the cards, and the banner and the pictures' credit beneath.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - `window`: the Y `injected` parameter.
    pub fn draw(game: &Game, highest: u32, window: &mut Window) {
        window.clear(palette::BACKGROUND);
        let mut number = 0;
        for row in 0..board::ROWS { // y: #row
            for column in 0..board::COLUMNS { // y: #column
                number += 1;
                draw_card(game, number, corner_of(row, column), window);
            }
        }
        let lines = banner(game, highest);
        let top = layout::HEIGHT - layout::BAR_HEIGHT;
        let credit = "Pictures: OpenMoji, CC BY-SA 4.0";
        window.write(&lines[0], layout::GAP, top + 24, layout::TEXT_SIZE, palette::TEXT);
        window.write(&lines[1], layout::GAP, top + 50, layout::TEXT_SIZE, palette::TEXT);
        window.write(credit, layout::GAP, top + 76, layout::TEXT_SIZE, palette::CREDIT);
    }
}

/// Draws one card: its back, or its face with its picture.
///
/// - `game`: the game.
/// - `number`: the card's number.
/// - `corner`: the card's top left corner.
/// - `window`: the Y `injected` parameter.
fn draw_card(game: &Game, number: u32, corner: Point, window: &mut Window) {
    match list::at_number(&game.sides, number) {
        Side::FaceDown => fill_card(corner, palette::BACK, window), // y: #shown.face_down
        Side::FaceUp => { // y: #shown.face_up
            fill_card(corner, palette::FACE, window);
            draw_picture(game, number, corner, window);
        }
        Side::Matched => { // y: #shown.matched
            fill_card(corner, palette::MATCHED, window);
            draw_picture(game, number, corner, window);
        }
    }
}

/// Fills the square of one card.
///
/// - `corner`: the card's top left corner.
/// - `shade`: the colour.
/// - `window`: the Y `injected` parameter.
fn fill_card(corner: Point, shade: Colour, window: &mut Window) {
    window.fill(corner.x, corner.y, layout::CARD_SIZE, layout::CARD_SIZE, shade);
}

/// Draws a card's picture.
///
/// - `game`: the game.
/// - `number`: the card's number.
/// - `corner`: the card's top left corner.
/// - `window`: the Y `injected` parameter.
fn draw_picture(game: &Game, number: u32, corner: Point, window: &mut Window) {
    let picture = list::at_number_ref(&game.pictures, list::at_number(&game.pairs, number));
    window.picture(picture, corner.x + layout::PICTURE_INSET, corner.y + layout::PICTURE_INSET);
}

/// Shows that the pictures are being fetched, which takes a moment, while the window would otherwise stand still.
///
/// - `window`, `forever`: the Y `injected` parameters.
async fn show_fetching(window: &mut Window, forever: &Forever) {
    window.clear(palette::BACKGROUND);
    let message = "Fetching pictures from OpenMoji...";
    window.write(message, layout::GAP, layout::HEIGHT / 2, layout::TEXT_SIZE, palette::TEXT);
    window.next_frame(forever).await; // y: forever because "a hidden window shows no frames, and the game waits until it is shown"
}

/// The Y facet `window_play`: playing in a window, one game after another, until the window is closed.
pub mod window_play {
    use super::*;

    /// Plays Memory in a window: one game after another, until the player closes the window or presses Escape. When
    /// a game ends, Space starts the next, with new pictures.
    ///
    /// - `window`, `clock`, `random`, `site`, `forever`: the Y `injected` parameters.
    /// - Returns the best score of the games played.
    /// - Fails `Unreachable`, `TimedOut`, `NotAPicture`: as `new_game` says.
    pub async fn play(window: &mut Window, clock: &mut Clock, random: &mut Random, site: &Http, forever: &Forever) -> Result<u32, SetupError> {
        show_fetching(window, forever).await;
        let mut game = new_game(random, clock, site).await?;
        let mut highest = 0;
        let mut quitting = false;
        while !quitting && !window.close_requested() { // y: #open, else #closed_at_once
            let keys = window.keys_pressed();
            let now = clock.now();
            quitting = keys.contains(&Key::Escape);
            let reached = standing(&game);
            if reached != Outcome::Playing && keys.contains(&Key::Space) { // y: #moment.again
                show_fetching(window, forever).await;
                game = new_game(random, clock, site).await?;
            } else if list::count(&game.turned) == 2 && now - game.turned_at >= game_rules::SHOW_TIME { // y: #moment.hiding
                turn_back(&mut game);
            } else if reached == Outcome::Playing { // y: #moment.picking
                game.elapsed = now - game.started;
                pick(&mut game, &window.clicks(), now);
            } else { // y: #moment.over
                highest = highest.max(score(&game));
            }
            draw(&game, highest, window);
            window.next_frame(forever).await; // y: forever because "a hidden window shows no frames, and the game waits until it is shown"
        }
        Ok(highest)
    }
}

/// Turns over the cards the player clicked, in the order clicked. A click that turns no card is ignored.
///
/// - `game`: the game, which changes.
/// - `clicks`: the points clicked.
/// - `now`: the time now.
fn pick(game: &mut Game, clicks: &[Point], now: Instant) {
    for place in clicks { // y: #place, else #no_clicks
        let choice = choosable(game, *place);
        match choice {
            Some(choice) => turn_over(game, choice, now), // y: #chosen, which narrows `choice` to a `uint32`
            None => continue, // y: #ignored
        }
    }
}
