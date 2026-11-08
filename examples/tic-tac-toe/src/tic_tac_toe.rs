//! Lowered from `y/src/tic_tac_toe.y`, the Y module `tic_tac_toe`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `tic_tac_toe::moves::read_move`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm.

use y_runtime::{ensure, list, not_reachable, require, ConsoleError, Console, Forever};
use {y_collections::List, y_string::YString};
use self::{console_play::*, moves::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the board, the marks on it, and how a game starts.
pub mod setup {
    use super::*;

    /// The board: the Y constant group `board`.
    pub mod board {
        /// How many squares the board has, numbered from 1 row by row from the top left.
        pub const SQUARES: u32 = 9;

        /// The board's eight lines of three squares: three rows, three columns and two diagonals.
        pub const LINES: [[u32; 3]; 8] = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [1, 4, 7], [2, 5, 8], [3, 6, 9], [1, 5, 9], [3, 5, 7]];
    }

    /// A mark a player puts in a square. The person plays crosses and moves first; the computer plays noughts.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Mark {
        Cross,
        Nought,
    }

    /// A constant of the group `mark` shows as its value (Y § 3.5).
    impl std::fmt::Display for Mark {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Mark::Cross => f.write_str("X"),
                Mark::Nought => f.write_str("O"),
            }
        }
    }

    /// One game.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Game {
        /// The squares, in the order they are numbered: the mark in each, or none.
        pub(super) squares: List<Option<Mark>>,
        /// Whose turn it is.
        pub(super) to_move: Mark,
    }

    /// Starts a game.
    ///
    /// - Returns an empty board, with crosses to move.
    pub fn new_game() -> Game {
        Game { squares: List::from(list::repeated(None, board::SQUARES)), to_move: Mark::Cross }
    }
}

/// The Y facet `moves`: reading moves, making them, and choosing one.
pub mod moves {
    use super::*;

    /// Why what the player typed is not a move.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MoveError {
        NotANumber,
        NoSuchSquare,
        Taken,
    }

    /// Reads a move from what the player typed: the number of an empty square.
    ///
    /// - `game`: the game.
    /// - `line`: what the player typed, such as `5`.
    /// - Returns the square's number.
    /// - Fails `NotANumber`: the line does not hold one whole number.
    /// - Fails `NoSuchSquare`: the number is not that of a square.
    /// - Fails `Taken`: the square already holds a mark.
    pub fn read_move(game: &Game, line: &YString) -> Result<u32, MoveError> {
        let number: Option<u32> = line.trimmed().parse_uint32();
        let Some(number) = number else { // y: #not_a_number, else #a_number
            return Err(MoveError::NotANumber);
        };
        if number == 0 || number > board::SQUARES { // y: #judged.off_board
            Err(MoveError::NoSuchSquare)
        } else if list::at_number(&game.squares, number).is_some() { // y: #judged.taken
            Err(MoveError::Taken)
        } else { // y: #judged.empty
            Ok(number)
        }
    }

    /// Puts the mark of the player to move in a square, and passes the turn to the other player.
    ///
    /// - `game`: the game, which changes.
    /// - `square`: the square's number.
    pub fn place(game: &mut Game, square: u32) {
        require(list::at_number(&game.squares, square).is_none(), "tic_tac_toe::place#unmarked");
        list::set_number(&mut game.squares, square, Some(game.to_move));
        game.to_move = other(game.to_move);
    }

    /// Chooses the computer's move: the first square, in the order they are numbered, whose value to the computer is the
    /// highest. With perfect play the computer never loses.
    ///
    /// - `game`: the game.
    /// - Returns the square's number.
    pub fn computer_move(game: &Game) -> u32 {
        require(!over(game), "tic_tac_toe::computer_move#still_playing");
        let open = open_squares(game);
        let mut best = -100;
        let mut found = open[0];
        let mut ran = false;
        for square in open { // y: #square
            ran = true;
            let mut after = game.clone(); // y: a copy, because place changes it and `game` is still read
            place(&mut after, square);
            let value = -value_of(&after);
            if value > best { // y: #better
                best = value;
                found = square;
            } else { // y: #no_better
                continue;
            }
        }
        if !ran { // y: #no_open
            not_reachable("tic_tac_toe::computer_move#no_open");
        }
        found
    }
}

/// Whether a game has ended: a player has three in a row, or the board is full.
///
/// - `game`: the game.
/// - Returns whether it has.
fn over(game: &Game) -> bool {
    winner(game).is_some() || list::count(&open_squares(game)) == 0
}

/// The player who has three in a row, if one has.
///
/// - `game`: the game.
/// - Returns that player's mark, or none.
fn winner(game: &Game) -> Option<Mark> {
    for line in board::LINES { // y: #line
        let first = list::at_number(&game.squares, line[0]);
        let second = list::at_number(&game.squares, line[1]);
        let third = list::at_number(&game.squares, line[2]);
        if first.is_some() && second == first && third == first { // y: #three
            return first;
        } else { // y: #not_three
            continue;
        }
    }
    None
}

/// The mark of the player who moves after the one with `player`.
///
/// - `player`: a mark.
/// - Returns the other mark.
fn other(player: Mark) -> Mark {
    match player {
        Mark::Cross => Mark::Nought, // y: #current.cross
        Mark::Nought => Mark::Cross, // y: #current.nought
    }
}

/// The squares that hold no mark, in the order they are numbered.
///
/// - `game`: the game.
/// - Returns their numbers.
fn open_squares(game: &Game) -> List<u32> {
    let mut found = List::new();
    for number in 1..=board::SQUARES { // y: #number
        if list::at_number(&game.squares, number).is_some() { // y: #marked
            continue;
        } else { // y: #empty
            found.push(number);
        }
    }
    found
}

/// The value of a game to the player whose turn it is, when both players play perfectly from here on: more than zero
/// for a win, zero for a draw, and less than zero for a loss. A win is worth more the sooner it comes, and a loss costs
/// more.
///
/// - `game`: the game.
/// - Returns the value, from -10 to 10.
fn value_of(game: &Game) -> i32 {
    let open = open_squares(game);
    let value = if winner(game).is_some() { // y: #now.lost
        -1 - list::count_int32(&open)
    } else if list::count(&open) == 0 { // y: #now.drawn
        0
    } else { // y: #now.open
        let mut best = -100;
        for square in open { // y: #square
            let mut after = game.clone(); // y: a copy, because place changes it and `game` is still read
            place(&mut after, square);
            best = best.max(-value_of(&after));
        }
        best
    };
    ensure((-10..=10).contains(&value), "tic_tac_toe::value_of#bounded");
    value
}

/// Draws the board, three rows of three squares. An empty square shows its number, so the player sees what to type.
///
/// - `game`: the game.
/// - Returns the drawing.
fn shown(game: &Game) -> YString {
    let mut rows: List<YString> = List::new();
    for first in [1, 4, 7] { // y: #first
        let mut row: List<YString> = List::new();
        for number in first..=first + 2 { // y: #number
            row.push(square_shown(game, number));
        }
        let line = YString::from(row.iter().map(YString::to_string).reduce(|a, b| a + " | " + &b).unwrap_or_default());
        rows.push(YString::from(format!(" {line}")));
    }
    YString::from(rows.iter().map(YString::to_string).reduce(|a, b| a + "\n---+---+---\n" + &b).unwrap_or_default())
}

/// How one square is drawn.
///
/// - `game`: the game.
/// - `number`: the square's number.
/// - Returns its mark, or its number when it has none.
fn square_shown(game: &Game, number: u32) -> YString {
    YString::from(match list::at_number(&game.squares, number) {
        None => number.to_string(), // y: #held.empty
        Some(held) => held.to_string(), // y: #held.marked, which narrows `held` to a `mark`
    })
}

/// The Y facet `console_play`: playing a whole game at the console.
pub mod console_play {
    use super::*;

    /// Why a game could not be played to its end.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PlayError {
        InputClosed,
        Console { error: ConsoleError },
    }

    /// Y's `or fail` on a `ConsoleError` converts through `PlayError`'s one case that carries it (Y § 3.4).
    impl From<ConsoleError> for PlayError {
        fn from(error: ConsoleError) -> PlayError {
            PlayError::Console { error }
        }
    }

    /// Plays a game at the console, the player's crosses against the computer's noughts, until a player has three in
    /// a row or the board is full.
    ///
    /// - `started`: the game as it starts.
    /// - `console`, `forever`: the Y `injected` parameters.
    /// - Returns nothing. The console shows the player how the game ended.
    /// - Fails `InputClosed`: the console closed before the game ended.
    /// - Fails `Console`: the console could not be read or written.
    pub fn play(started: Game, console: &mut Console, forever: &Forever) -> Result<(), PlayError> {
        let mut game = started;
        console.write_line("Tic-tac-toe: you are X, and move first. Three in a row wins.")?;
        while !over(&game) { // y: #playing, else #over_at_once
            if game.to_move == Mark::Cross { // y: #persons_turn
                persons_move(&mut game, console, forever)?;
            } else { // y: #computers_turn
                let chosen = computer_move(&game);
                place(&mut game, chosen);
                console.write_line(&format!("The computer plays {chosen}."))?;
            }
        }
        announce(&game, console)?;
        Ok(())
    }
}

/// Shows the board and reads the player's move, then makes it, or says what is wrong with what they typed.
///
/// - `game`: the game, which changes when the move is made.
/// - `console`, `forever`: the Y `injected` parameters.
/// - Fails `InputClosed`: the console closed before the player moved.
/// - Fails `Console`: the console could not be read or written.
fn persons_move(game: &mut Game, console: &mut Console, forever: &Forever) -> Result<(), PlayError> {
    let picture = shown(game);
    console.write_line(picture.as_str())?;
    console.write("Your move (a square's number): ")?;
    let typed = console.read_line(forever)?; // y: forever because "a player may think for as long as they like"
    let Some(typed) = typed else { // y: #closed, else #line_read, which narrows `typed` to a `string`
        return Err(PlayError::InputClosed);
    };
    match read_move(game, &YString::from(typed)) {
        Ok(chosen) => place(game, chosen), // y: #attempted.valid
        Err(error) => console.write_line(complaint(error).as_str())?, // y: #attempted.invalid
    }
    Ok(())
}

/// What the game says to what is not a move.
///
/// - `error`: why it is not.
/// - Returns the line to write.
fn complaint(error: MoveError) -> YString {
    YString::new(match error {
        MoveError::NotANumber => "Type the number of a square, from 1 to 9.", // y: #wrong.not_a_move
        MoveError::NoSuchSquare => "There is no such square.", // y: #wrong.no_square
        MoveError::Taken => "That square is taken.", // y: #wrong.taken
    })
}

/// Shows the board as the game ended, and how it ended.
///
/// - `game`: the game.
/// - `console`: the Y `injected` parameter.
/// - Fails with a `ConsoleError`: the console could not be written.
fn announce(game: &Game, console: &mut Console) -> Result<(), ConsoleError> {
    let picture = shown(game);
    console.write_line(picture.as_str())?;
    let found = winner(game);
    match found {
        None => console.write_line("The board is full. A draw.")?, // y: #drawn
        Some(found) => console.write_line(&format!("{found} has three in a row, and wins."))?, // y: #won, which narrows `found` to a `mark`
    }
    Ok(())
}

