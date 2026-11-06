//! Lowered from `y/src/nim.y`, the Y module `nim`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `nim::moves::read_move`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` on its `match` arm.

use y_collections::List;
use y_runtime::{list, not_reachable, require, ConsoleError, Console, Forever};
use y_string::YString;
use self::{console_play::*, moves::*, setup::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: a game, and how it starts.
mod setup {
    use y_collections::List;

    /// The rules every game is played by: the Y constant group `game_rules`.
    mod game_rules {
        /// How many counters each heap holds at the start, from the first heap to the last.
        const STARTING_HEAPS = [3, 4, 5];
    }

    /// One game: how many counters are left in each heap.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Game {
        heaps,
    }

    /// Who is playing.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Player {
        Person,
        Computer,
    }

    /// Starts a game.
    ///
    /// - Returns a game with every heap full.
    fn new_game() {
        Game { heaps: List::from(game_rules::STARTING_HEAPS) }
    }
}

/// The Y facet `moves`: reading moves, checking them, making them, and choosing one.
mod moves {
    use super::*;

    /// A move: which heap, numbered from 1 as the player sees it, and how many counters to take from it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Move {
        heap,
        take,
    }

    /// Why what the player typed is not a move.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MoveError {
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
    fn parse_move(line) {
        let pieces = List::from(line.split_whitespace());
        if pieces.count() != 2 { // y: #not_two, else #two
            return Err(MoveError::NotTwoNumbers);
        }
        let heap = pieces[0].parse_uint32();
        let take = pieces[1].parse_uint32();
        // y: the `or` of the Y source's `if not_numbers` narrows both names; a `match` tests its operands in order
        match (heap, take) {
            (None, _) => Err(MoveError::NotTwoNumbers), // y: #not_numbers'1
            (_, None) => Err(MoveError::NotTwoNumbers), // y: #not_numbers'2
            (Some(heap), Some(take)) => Ok(Move { heap, take }), // y: #numbers, which narrows `heap` and `take` to `uint32`s
        }
    }

    /// Checks that a move can be made in a game.
    ///
    /// - `game`: the game.
    /// - `chosen`: the move.
    /// - Returns the same move.
    /// - Fails `NoSuchHeap`: the game has no heap with that number.
    /// - Fails `NothingTaken`: the move takes no counters.
    /// - Fails `TooMany`: the heap holds fewer counters than the move takes.
    fn check(game, chosen) {
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
    fn read_move(game, line) {
        let parsed = parse_move(line)?;
        check(game, &parsed)
    }

    /// Makes a move.
    ///
    /// - `game`: the game, which loses the counters taken.
    /// - `chosen`: the move.
    fn remove(game, chosen) {
        require(check(game, chosen).is_ok(), "nim::remove#allowed");
        let heap = list::at_number(&game.heaps, chosen.heap);
        list::set_number(&mut game.heaps, chosen.heap, heap - chosen.take);
    }

    /// Chooses the computer's move. It leaves a nim-sum of zero when it can, which wins every game that can be won.
    ///
    /// - `game`: the game.
    /// - Returns the move.
    fn computer_move(game) {
        require(counters_left(game) > 0, "nim::computer_move#still_playing");
        let sum = nim_sum(&game.heaps);
        if sum == 0 { // y: #lost
            let largest = list::largest(&game.heaps);
            Move { heap: list::number_of(&game.heaps, &largest), take: 1 }
        } else { // y: #winnable
            let ran = false;
            for number in 1..=list::count(&game.heaps) { // y: #number
                ran = true;
                let heap = list::at_number(&game.heaps, number);
                let target = heap ^ sum;
                if target < heap { // y: #winning
                    return Move { heap: number, take: heap - target };
                } else { // y: #not_this_heap
                    continue;
                }
            }
            if !ran { // y: #no_heaps
                not_reachable("nim::computer_move#no_heaps");
            }
            not_reachable("nim::computer_move#winnable")
        }
    }
}

/// How many counters are left in the game.
///
/// - `game`: the game.
/// - Returns the counters in every heap together.
fn counters_left(game) {
    let total = 0;
    for heap in &game.heaps { // y: #heap, else #no_heaps
        total += heap;
    }
    total
}

/// The nim-sum of the heaps: their sizes combined with exclusive or.
///
/// - `heaps`: the heaps.
/// - Returns the nim-sum, which is zero exactly when the player to move loses against perfect play.
fn nim_sum(heaps) {
    let total = 0;
    for heap in heaps { // y: #heap, else #no_heaps
        total ^= heap;
    }
    total
}

/// Draws the heaps, one line each, with a bar for each counter.
///
/// - `game`: the game.
/// - Returns the drawing.
fn shown(game) {
    let lines = List::new();
    let number = 0;
    for heap in &game.heaps { // y: #heap, else #no_heaps
        number += 1;
        let counters = YString::from(list::repeated("|", *heap).concat());
        lines.push(YString::from(format!("  {number}: {counters}")));
    }
    let pieces = lines.iter().map(YString::as_str).collect();
    YString::from(pieces.join("\n"))
}

/// The Y facet `console_play`: playing a whole game at the console.
mod console_play {
    use super::*;

    /// Why a game could not be played to its end.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum PlayError {
        InputClosed,
        Console { error },
    }

    /// Y's `or fail` on a `ConsoleError` converts through `PlayError`'s one case that carries it (Y § 3.4).
    impl From for PlayError {
        fn from(error) {
            PlayError::Console { error }
        }
    }

    /// Plays a game at the console, the player against the computer, until the last counter is taken. The player
    /// moves first.
    ///
    /// - `started`: the game as it starts.
    /// - `console`, `forever`: the Y `injected` parameters.
    /// - Returns nothing. The console shows the player how the game ended.
    /// - Fails `InputClosed`: the console closed before the game ended.
    /// - Fails `Console`: the console could not be read or written.
    fn play(started, console, forever) {
        let game = started;
        let turn = Player::Person;
        console.write_line("Nim: take any number of counters from one heap. Whoever takes the last counter wins.")?;
        while counters_left(&game) > 0 { // y: #playing, else #nothing_to_play
            let picture = shown(&game);
            console.write_line(picture.as_str())?;
            if turn == Player::Person { // y: #persons_turn
                turn = persons_move(&mut game, console, forever)?;
            } else { // y: #computers_turn
                let chosen = computer_move(&game);
                remove(&mut game, &chosen);
                console.write_line(&format!("The computer takes {} from heap {}.", chosen.take, chosen.heap))?;
                turn = Player::Person;
            }
        }
        announce(turn, console)?;
        Ok(())
    }
}

/// Reads the player's move and makes it, or says what is wrong with what they typed.
///
/// - `game`: the game, which loses the counters taken when the move is made.
/// - `console`, `forever`: the Y `injected` parameters.
/// - Returns who moves next: the computer once a move is made, and the player again otherwise.
/// - Fails `InputClosed`: the console closed before the player moved.
/// - Fails `Console`: the console could not be read or written.
fn persons_move(game, console, forever) {
    console.write("Your move (heap, then counters): ")?;
    let typed = console.read_line(forever)?; // y: forever because "a player may think for as long as they like"
    let Some(typed) = typed else { // y: #closed, else #line_read, which narrows `typed` to a `string`
        return Err(PlayError::InputClosed);
    };
    match read_move(game, &YString::from(typed)) {
        Ok(chosen) => { // y: #attempted.valid
            remove(game, &chosen);
            Ok(Player::Computer)
        }
        Err(error) => { // y: #attempted.invalid
            console.write_line(complaint(error).as_str())?;
            Ok(Player::Person)
        }
    }
}

/// What the game says to what is not a move.
///
/// - `error`: why it is not.
/// - Returns the line to write.
fn complaint(error) {
    YString::new(match error {
        MoveError::NotTwoNumbers => "Type a heap's number and how many to take, such as 2 3.", // y: #wrong.not_a_move
        MoveError::NoSuchHeap => "There is no such heap.", // y: #wrong.no_heap
        MoveError::NothingTaken => "Take at least one counter.", // y: #wrong.none_taken
        MoveError::TooMany => "That heap does not hold that many.", // y: #wrong.too_many
    })
}

/// Says who won.
///
/// - `turn`: whose turn it is once the last counter is taken: the other player's than the winner's.
/// - `console`: the Y `injected` parameter.
/// - Fails with a `ConsoleError`: the console could not be written.
fn announce(turn, console) {
    if turn == Player::Person { // y: #computer_won
        console.write_line("The computer took the last counter. The computer wins.")?;
    } else { // y: #person_won
        console.write_line("You took the last counter. You win!")?;
    }
    Ok(())
}

