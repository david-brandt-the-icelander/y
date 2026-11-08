//! Lowered from `y/src/memory.y`, the Y module `memory`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `memory::rules::turn_over`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm or its `if`.
//! `new_game` may suspend, because `site.get()` may, and `play` may, because it calls `new_game` and
//! `window.next_frame()`: Y has no `async`, and Rust's colouring makes them `async fn`s.

use std::time::{Duration, Instant};
use y_collections::{list, List};
use y_http::{Http, HttpError};
use y_runtime::{ensure, list, require, time, Clock, Deadline, Expired, Forever, Random, Within};
use y_string::YString;
use y_window::{Colour, Key, Picture, PictureError, Point, Window};
use self::{drawing::*, rules::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the board, the rules, the pictures, and how a game starts.
mod setup {
    use super::*;

    /// The board: the Y constant group `board`.
    mod board {
        /// How many cards are laid out across the board, and how many down it.
        const COLUMNS = 5;
        const ROWS = 4;
        /// How many cards there are, numbered from 1 row by row from the top left.
        const CARDS = 20;
        /// How many pairs of cards there are: one for each picture.
        const PAIRS = 10;
    }

    /// The rules every game is played by, and how it is scored: the Y constant group `game_rules`.
    mod game_rules {
        use std::time::Duration;

        /// How many times the player may turn over two cards that do not match before the game is lost.
        const LIVES = 10;
        /// How long two cards that do not match stay face up.
        const SHOW_TIME = Duration::from_secs(1);
        /// How long the pictures may take to fetch.
        const FETCH_TIME = Duration::from_secs(20);
        /// What each pair found scores.
        const POINTS_PER_PAIR = 100;
        /// What each life left scores, when every pair is found.
        const POINTS_PER_LIFE = 50;
        /// What each whole second under the par time scores, when every pair is found.
        const POINTS_PER_SECOND = 10;
        /// The time to beat.
        const PAR_TIME = Duration::from_secs(120);
    }

    /// The pictures the cards may show: the Y constant group `faces`.
    mod faces {
        /// The OpenMoji codes of twenty animals' faces, from which each game chooses its pictures.
        const CODES = [
            "1F436", "1F431", "1F42D", "1F439", "1F430", "1F98A", "1F43B", "1F43C", "1F428", "1F42F", "1F981", "1F42E",
            "1F437", "1F438", "1F435", "1F414", "1F427", "1F426", "1F989", "1F984",
        ];
    }

    /// OpenMoji, the open-source emoji project, whose pictures the cards show: the Y service `openmoji`. Its pictures
    /// are licensed CC BY-SA 4.0, and the game credits them.
    mod openmoji {
        const ADDRESS = "https://openmoji.org";
    }

    /// Which side of a card is up.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Side {
        FaceDown,
        FaceUp,
        Matched,
    }

    /// Where a game stands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Playing,
        Won,
        Lost,
    }

    /// One game.
    #[derive(Clone, Debug, PartialEq)]
    struct Game {
        /// For each card, in the order they are numbered, the number of the picture it shows.
        pairs,
        /// For each card, in the same order, which side is up.
        sides,
        /// The pictures, numbered from 1.
        pictures,
        /// The cards turned face up this turn, and not yet matched: none, one or two.
        turned,
        /// When the second card of the turn was turned, or when the game started.
        turned_at,
        /// How many pairs have been found.
        matches,
        /// How many lives are left.
        lives,
        /// When the game started.
        started,
        /// How long the game has been played, which stops when it ends.
        elapsed,
    }

    /// Why a game could not be set up.
    #[derive(Clone, Debug, PartialEq)]
    enum SetupError {
        Unreachable { error },
        TimedOut { deadline },
        NotAPicture { error },
    }

    /// Y's `or fail` inside `within` converts the call's own failure, or the expiry, through `SetupError`'s case that
    /// carries it (Y § 3.4, § 3.8).
    impl From for SetupError {
        fn from(failed) {
            match failed {
                Within::Failed(error) => SetupError::Unreachable { error },
                Within::Expired(deadline) => SetupError::TimedOut { deadline },
            }
        }
    }

    /// Y's `or fail` on a `PictureError` converts through `SetupError`'s one case that carries it (Y § 3.4).
    impl From for SetupError {
        fn from(error) {
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
    async fn new_game(random, clock, site) {
        let codes = List::from(random.chosen(&faces::CODES.map(YString::new), board::PAIRS));
        let pictures = List::new();
        {
            // y: region fetch is within game_rules.fetch_time or fail
            let deadline = Deadline::after(game_rules::FETCH_TIME);
            for code in codes { // y: #code
                let drawing = site.get(&format!("/data/color/svg/{code}.svg"), &deadline).await?;
                let drawing = YString::from(drawing);
                let made = Picture::from_svg(drawing.as_str(), layout::PICTURE_SIZE)?;
                pictures.push(made);
            }
        }
        let dealt = List::new();
        for number in 1..=board::PAIRS { // y: #number
            dealt.push(number);
            dealt.push(number);
        }
        let now = clock.now();
        let started = Game {
            pairs: List::from(random.shuffled(&dealt)),
            sides: List::from(list::repeated(Side::FaceDown, board::CARDS)),
            pictures,
            turned: List::new(),
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
mod rules {
    use super::*;

    /// The card the player may turn over by clicking a point, if there is one.
    ///
    /// - `game`: the game.
    /// - `place`: the point clicked.
    /// - Returns the card's number, or none when the point is not on a card, the card is face up, or two cards are
    ///   already face up.
    fn choosable(game, place) {
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
    fn turn_over(game, number, now) {
        require(list::at_number(&game.sides, number) == Side::FaceDown && list::count(&game.turned) < 2, "memory::turn_over#may_turn");
        if game.turned.is_empty() { // y: #turn.first
            list::set_number(&mut game.sides, number, Side::FaceUp);
            game.turned.push(number);
        } else if list::at_number(&game.pairs, game.turned[0]) == list::at_number(&game.pairs, number) { // y: #turn.matching
            list::set_number(&mut game.sides, game.turned[0], Side::Matched);
            list::set_number(&mut game.sides, number, Side::Matched);
            game.turned = List::new();
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
    fn turn_back(game) {
        require(list::count(&game.turned) == 2, "memory::turn_back#two_turned");
        for number in &game.turned { // y: #number
            list::set_number(&mut game.sides, *number, Side::FaceDown);
        }
        game.turned = List::new();
    }

    /// Where a game stands: won when every pair is found, lost when no life is left, and otherwise still being played.
    ///
    /// - `game`: the game.
    /// - Returns where it stands.
    fn standing(game) {
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
    fn score(game) {
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
fn time_bonus(elapsed) {
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
fn card_at(place) {
    let number = 0;
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
fn corner_of(row, column) {
    Point { x: layout::GAP + column * layout::STEP, y: layout::GAP + row * layout::STEP }
}

/// The Y facet `drawing`: what the player sees of a game.
mod drawing {
    use super::*;

    /// The sizes of what is drawn, in pixels: the Y constant group `layout`.
    mod layout {
        use super::board;

        /// The side of a card.
        const CARD_SIZE = 112;
        /// The space between two cards, and around them.
        const GAP = 12;
        /// From one card to the next.
        const STEP = CARD_SIZE + GAP;
        /// How far a picture is drawn in from its card's edge.
        const PICTURE_INSET = 8;
        /// The side of a picture: the card's, less the inset on both sides.
        const PICTURE_SIZE = 96;
        /// The height of the bar beneath the board, where the score is written.
        const BAR_HEIGHT = 84;
        /// The height of the text.
        const TEXT_SIZE = 20;
        const WIDTH = GAP + board::COLUMNS * STEP;
        const HEIGHT = GAP + board::ROWS * STEP + BAR_HEIGHT;
    }

    /// The colours of what is drawn: the Y constant group `palette`.
    mod palette {
        use super::Colour;

        const BACKGROUND = Colour::rgb(24, 26, 33);
        const BACK = Colour::rgb(70, 90, 140);
        const FACE = Colour::rgb(235, 235, 225);
        const MATCHED = Colour::rgb(170, 215, 160);
        const TEXT = Colour::rgb(230, 230, 230);
        const CREDIT = Colour::rgb(140, 140, 150);
    }

    /// The lines written beneath the board: how the game stands, and what the player can do.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - Returns the two lines.
    fn banner(game, highest) {
        let seconds = time::whole_seconds(game.elapsed);
        match standing(game) {
            Outcome::Playing => list![ // y: #shown.playing
                YString::from(format!("Pairs {} of {}   Lives {}   Time {seconds}s   Best {highest}", game.matches, board::PAIRS, game.lives)),
                YString::new("Click two cards to turn them over. Escape quits."),
            ],
            Outcome::Won => list![ // y: #shown.won
                YString::from(format!("All pairs in {seconds}s! Score {}   Best {highest}", score(game))),
                YString::new("Space plays again. Escape quits."),
            ],
            Outcome::Lost => list![ // y: #shown.lost
                YString::from(format!("Out of lives with {} pairs. Score {}   Best {highest}", game.matches, score(game))),
                YString::new("Space plays again. Escape quits."),
            ],
        }
    }

    /// Draws a game: the cards, and the banner and the pictures' credit beneath.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - `window`: the Y `injected` parameter.
    fn draw(game, highest, window) {
        window.clear(palette::BACKGROUND);
        let number = 0;
        for row in 0..board::ROWS { // y: #row
            for column in 0..board::COLUMNS { // y: #column
                number += 1;
                draw_card(game, number, corner_of(row, column), window);
            }
        }
        let lines = banner(game, highest);
        let top = layout::HEIGHT - layout::BAR_HEIGHT;
        let credit = YString::new("Pictures: OpenMoji, CC BY-SA 4.0");
        window.write(lines[0].as_str(), layout::GAP, top + 24, layout::TEXT_SIZE, palette::TEXT);
        window.write(lines[1].as_str(), layout::GAP, top + 50, layout::TEXT_SIZE, palette::TEXT);
        window.write(credit.as_str(), layout::GAP, top + 76, layout::TEXT_SIZE, palette::CREDIT);
    }
}

/// Draws one card: its back, or its face with its picture.
///
/// - `game`: the game.
/// - `number`: the card's number.
/// - `corner`: the card's top left corner.
/// - `window`: the Y `injected` parameter.
fn draw_card(game, number, corner, window) {
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
fn fill_card(corner, shade, window) {
    window.fill(corner.x, corner.y, layout::CARD_SIZE, layout::CARD_SIZE, shade);
}

/// Draws a card's picture.
///
/// - `game`: the game.
/// - `number`: the card's number.
/// - `corner`: the card's top left corner.
/// - `window`: the Y `injected` parameter.
fn draw_picture(game, number, corner, window) {
    let picture = list::at_number_ref(&game.pictures, list::at_number(&game.pairs, number));
    window.picture(picture, corner.x + layout::PICTURE_INSET, corner.y + layout::PICTURE_INSET);
}

/// Shows that the pictures are being fetched, which takes a moment, while the window would otherwise stand still.
///
/// - `window`, `forever`: the Y `injected` parameters.
async fn show_fetching(window, forever) {
    window.clear(palette::BACKGROUND);
    let message = YString::new("Fetching pictures from OpenMoji...");
    window.write(message.as_str(), layout::GAP, layout::HEIGHT / 2, layout::TEXT_SIZE, palette::TEXT);
    window.next_frame(forever).await; // y: region frame_shown is forever because "a hidden window shows no frames, and the game waits until it is shown"
}

/// The Y facet `window_play`: playing in a window, one game after another, until the window is closed.
mod window_play {
    use super::*;

    /// Plays Memory in a window: one game after another, until the player closes the window or presses Escape. When
    /// a game ends, Space starts the next, with new pictures.
    ///
    /// - `window`, `clock`, `random`, `site`, `forever`: the Y `injected` parameters.
    /// - Returns the best score of the games played.
    /// - Fails `Unreachable`, `TimedOut`, `NotAPicture`: as `new_game` says.
    async fn play(window, clock, random, site, forever) {
        show_fetching(window, forever).await;
        let game = new_game(random, clock, site).await?;
        let highest = 0;
        let quitting = false;
        while !quitting && !window.close_requested() { // y: #open, else #closed_at_once
            let keys = List::from(window.keys_pressed());
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
                pick(&mut game, &List::from(window.clicks()), now);
            } else { // y: #moment.over
                highest = highest.max(score(&game));
            }
            draw(&game, highest, window);
            window.next_frame(forever).await; // y: region frame_shown is forever because "a hidden window shows no frames, and the game waits until it is shown"
        }
        Ok(highest)
    }
}

/// Turns over the cards the player clicked, in the order clicked. A click that turns no card is ignored.
///
/// - `game`: the game, which changes.
/// - `clicks`: the points clicked.
/// - `now`: the time now.
fn pick(game, clicks, now) {
    for place in clicks { // y: #place, else #no_clicks
        let choice = choosable(game, *place);
        match choice {
            Some(choice) => turn_over(game, choice, now), // y: #chosen, which narrows `choice` to a `uint32`
            None => continue, // y: #ignored
        }
    }
}
