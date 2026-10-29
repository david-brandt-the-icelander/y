//! Lowered from `y/src/nim.y`, the Y module `nim`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `nim::moves::read_move`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm.

use y_runtime::{list, require, text, ConsoleError, Console, Forever};
use self::{moves::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: a game, and how it starts.
pub mod setup {
    /// The rules every game is played by: the Y constant group `game_rules`.
    pub mod game_rules {
        /// How many counters each heap holds at the start, from the first heap to the last.
        pub const STARTING_HEAPS: [u32; 3] = [3, 4, 5];
    }

    /// One game: how many counters are left in each heap.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Game {
        pub(super) heaps: Vec<u32>,
    }

    /// Who is playing.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Player {
        Person,
        Computer,
    }

    /// Starts a game.
    ///
    /// - Returns a game with every heap full.
    pub fn new_game() -> Game {
        Game { heaps: game_rules::STARTING_HEAPS.to_vec() }
    }
}

/// The Y facet `moves`: reading moves, checking them, making them, and choosing one.
pub mod moves {
    use super::*;

    /// A move: which heap, numbered from 1 as the player sees it, and how many counters to take from it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Move {
        pub(super) heap: u32,
        pub(super) take: u32,
    }

    /// Why what the player typed is not a move.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MoveError {
        NotTwoNumbers,
        NoSuchHeap,
        NothingTaken,
        TooMany,
    }

    /// Reads a move from what the player typed: the heap's number, then how many counters to take.
    ///
    /// - `line`: what the player typed, such as `2 3`.
    /// - Returns the move, not yet checked against a game.
    /// - Fails `NotTwoNumbers`: the line does not hold exactly two whole numbers.
    pub fn parse_move(line: &str) -> Result<Move, MoveError> {
        let pieces: Vec<&str> = line.split_whitespace().collect();
        if pieces.len() != 2 { // y: #not_two, else #two
            return Err(MoveError::NotTwoNumbers);
        }
        let heap: Option<u32> = pieces[0].parse().ok();
        let take: Option<u32> = pieces[1].parse().ok();
        let (Some(heap), Some(take)) = (heap, take) else { // y: #not_numbers, else #numbers
            return Err(MoveError::NotTwoNumbers);
        };
        Ok(Move { heap, take })
    }

    /// Checks that a move can be made in a game.
    ///
    /// - `game`: the game.
    /// - `chosen`: the move.
    /// - Returns the same move.
    /// - Fails `NoSuchHeap`: the game has no heap with that number.
    /// - Fails `NothingTaken`: the move takes no counters.
    /// - Fails `TooMany`: the heap holds fewer counters than the move takes.
    pub fn check(game: &Game, chosen: &Move) -> Result<Move, MoveError> {
        if chosen.heap == 0 || chosen.heap > list::count(&game.heaps) { // y: #no_such_heap, else #heap_exists
            return Err(MoveError::NoSuchHeap);
        }
        let heap = list::at_number(&game.heaps, chosen.heap);
        if chosen.take == 0 { // y: #judged.nothing_taken
            Err(MoveError::NothingTaken)
        } else if chosen.take > heap { // y: #judged.too_many
            Err(MoveError::TooMany)
        } else { // y: #judged.fits
            Ok(*chosen)
        }
    }

    /// Reads a move from what the player typed, and checks it against the game.
    ///
    /// - `game`: the game.
    /// - `line`: what the player typed.
    /// - Returns the move, which can be made.
    /// - Fails `NotTwoNumbers`, `NoSuchHeap`, `NothingTaken`, `TooMany`: as `parse_move` and `check` say.
    pub fn read_move(game: &Game, line: &str) -> Result<Move, MoveError> {
        let parsed = parse_move(line)?;
        check(game, &parsed)
    }

    /// Makes a move.
    ///
    /// - `game`: the game, which loses the counters taken.
    /// - `chosen`: the move.
    pub fn remove(game: &mut Game, chosen: &Move) {
        require(check(game, chosen).is_ok(), "nim::remove#allowed");
        let heap = list::at_number(&game.heaps, chosen.heap);
        list::set_number(&mut game.heaps, chosen.heap, heap - chosen.take);
    }

    /// Chooses the computer's move. It leaves a nim-sum of zero when it can, which wins every game that can be won.
    ///
    /// - `game`: the game.
    /// - Returns the move.
    pub fn computer_move(game: &Game) -> Move {
        require(counters_left(game) > 0, "nim::computer_move#still_playing");
        let sum = nim_sum(&game.heaps);
        let mut number = 1;
        while number <= list::count(&game.heaps) { // y: #searching
            let heap = list::at_number(&game.heaps, number);
            let target = heap ^ sum;
            if target < heap { // y: #winning
                return Move { heap: number, take: heap - target };
            } else { // y: #not_this_heap
                number += 1;
            }
        }
        // y: #losing, the `else` of `while searching`: the loop has no `break`, so it runs when the loop ends
        let largest = list::largest(&game.heaps);
        Move { heap: list::number_of(&game.heaps, &largest), take: 1 }
    }
}

/// How many counters are left in the game.
///
/// - `game`: the game.
/// - Returns the counters in every heap together.
fn counters_left(game: &Game) -> u32 {
    let mut total = 0;
    for heap in &game.heaps {
        total += heap;
    }
    total
}

/// The nim-sum of the heaps: their sizes combined with exclusive or.
///
/// - `heaps`: the heaps.
/// - Returns the nim-sum, which is zero exactly when the player to move loses against perfect play.
fn nim_sum(heaps: &[u32]) -> u32 {
    let mut total = 0;
    for heap in heaps {
        total ^= heap;
    }
    total
}

/// Draws the heaps, one line each, with a bar for each counter.
///
/// - `game`: the game.
/// - Returns the drawing.
fn shown(game: &Game) -> String {
    let mut lines = Vec::new();
    let mut number = 0;
    for heap in &game.heaps {
        number += 1;
        let counters = text::repeated("|", *heap);
        lines.push(format!("  {number}: {counters}"));
    }
    lines.join("\n")
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

    /// Plays one game at the console, the player against the computer, until the last counter is taken. The player
    /// moves first.
    ///
    /// - `console`, `forever`: the Y `injected` parameters.
    /// - Returns nothing. The console shows the player how the game ended.
    /// - Fails `InputClosed`: the console closed before the game ended.
    /// - Fails `Console`: the console could not be read or written.
    pub fn play(console: &mut Console, forever: &Forever) -> Result<(), PlayError> {
        let mut game = new_game();
        let mut turn = Player::Person;
        console.write_line("Nim: take any number of counters from one heap. Whoever takes the last counter wins.")?;
        while counters_left(&game) > 0 { // y: #playing
            let picture = shown(&game);
            console.write_line(&picture)?;
            if turn == Player::Person { // y: #persons_turn
                console.write("Your move (heap, then counters): ")?;
                let typed = console.read_line(forever)?; // y: forever because "a player may think for as long as they like"
                let Some(typed) = typed else { // y: #closed, else #line_read, which narrows `typed` to a `string`
                    return Err(PlayError::InputClosed);
                };
                match read_move(&game, &typed) {
                    Ok(chosen) => { // y: #attempted.valid
                        remove(&mut game, &chosen);
                        turn = Player::Computer;
                    }
                    Err(MoveError::NotTwoNumbers) => console.write_line("Type a heap's number and how many to take, such as 2 3.")?, // y: #attempted.not_a_move
                    Err(MoveError::NoSuchHeap) => console.write_line("There is no such heap.")?, // y: #attempted.no_heap
                    Err(MoveError::NothingTaken) => console.write_line("Take at least one counter.")?, // y: #attempted.none_taken
                    Err(MoveError::TooMany) => console.write_line("That heap does not hold that many.")?, // y: #attempted.too_many
                }
            } else { // y: #computers_turn
                let chosen = computer_move(&game);
                remove(&mut game, &chosen);
                console.write_line(&format!("The computer takes {} from heap {}.", chosen.take, chosen.heap))?;
                turn = Player::Person;
            }
        }
        // y: #game_over, the `else` of `while playing`: the loop has no `break`, so it runs when the loop ends
        if turn == Player::Person { // y: #computer_won
            console.write_line("The computer took the last counter. The computer wins.")?;
        } else { // y: #person_won
            console.write_line("You took the last counter. You win!")?;
        }
        Ok(())
    }
}
