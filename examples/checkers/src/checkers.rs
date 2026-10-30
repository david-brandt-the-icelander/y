//! Lowered from `y/src/checkers.y`, the Y module `checkers`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `checkers::rules::apply`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, and each named `condition`
//! of a `conditional` or a `compare` on its `match` arm or its `if`. A loop is marked with the name of a pass, and with
//! its `else`, which names the case in which it never runs: `// y: #placed, else #no_pieces`. An `else` with a body is
//! lowered with a flag, `ran`, set by the first pass.
//! `play` may suspend, because `window.next_frame()` may, and so may `choose_level` and `play_game`: Y has no `async`,
//! and Rust's colouring makes them `async fn`s.

use std::cmp::Ordering;
use y_runtime::{ensure, list, not_reachable, number, require, Forever, Random};
use y_window::{Colour, Key, Point, Window};
use self::{computer::*, drawing::*, rules::*, setup::*, window_play::*};

#[cfg(test)]
mod tests;

/// The Y facet `setup`: the board, the pieces, the levels of play, and how a game starts.
pub mod setup {
    use super::*;

    /// The board: the Y constant group `board`.
    pub mod board {
        /// How many squares there are along each side of the board.
        pub const SIZE: i32 = 8;
        /// How many rows of men each player starts with.
        pub const MEN_ROWS: i32 = 3;
    }

    /// The rules every game is played by: the Y constant group `game_rules`.
    pub mod game_rules {
        /// How many turns in a row may pass without a capture before the game is drawn.
        pub const QUIET_LIMIT: u32 = 80;
    }

    /// How a game won is scored: the Y constant group `scoring`.
    pub mod scoring {
        /// What a win scores before the player's moves are taken off, and before the level multiplies it.
        pub const WIN_POINTS: u32 = 1000;
        /// What each of the player's moves takes off.
        pub const POINTS_PER_MOVE: u32 = 10;
        /// The least a win scores, however many moves it took, before the level multiplies it.
        pub const LEAST_WIN: u32 = 100;
    }

    /// The two players. The person plays red, at the bottom of the board, and moves first; the computer plays white.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Side {
        Red,
        White,
    }

    /// One piece on the board.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Piece {
        /// The player whose piece it is.
        pub(super) owner: Side,
        /// Whether it has been crowned: a king moves backwards as well as forwards.
        pub(super) king: bool,
    }

    /// A square of the board, its row and its column each counted from 0 at the top left.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Square {
        pub(super) row: i32,
        pub(super) column: i32,
    }

    /// A piece and the square it stands on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Placed {
        pub(super) square: Square,
        pub(super) piece: Piece,
    }

    /// A diagonal way across the board: how many rows and how many columns one step moves.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Direction {
        pub(super) rows: i32,
        pub(super) columns: i32,
    }

    /// The ways a piece may move: the Y constant group `directions`.
    pub mod directions {
        use super::Direction;

        /// Towards the top of the board, the way red's men move.
        pub const UP: [Direction; 2] = [Direction { rows: -1, columns: -1 }, Direction { rows: -1, columns: 1 }];
        /// Towards the bottom of the board, the way white's men move.
        pub const DOWN: [Direction; 2] = [Direction { rows: 1, columns: -1 }, Direction { rows: 1, columns: 1 }];
        /// Every way, as kings move.
        pub const ALL: [Direction; 4] = [
            Direction { rows: -1, columns: -1 },
            Direction { rows: -1, columns: 1 },
            Direction { rows: 1, columns: -1 },
            Direction { rows: 1, columns: 1 },
        ];
    }

    /// One move: a step to a square next to the piece, or a chain of one or more jumps, each over a piece of the other
    /// player's, which is captured.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Move {
        /// The piece that moves.
        pub(super) piece: Piece,
        /// The square it moves from.
        pub(super) from: Square,
        /// The squares it lands on, in order; the last is where it ends.
        pub(super) path: Vec<Square>,
        /// The squares of the pieces it jumps over, in order: none for a step.
        pub(super) captures: Vec<Square>,
    }

    /// How hard the computer plays.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Level {
        /// What the player sees it called.
        pub(super) name: &'static str,
        /// How many positions the computer may look at to choose each of its moves.
        pub(super) checks: u32,
        /// How many times over a win at this level scores.
        pub(super) weight: u32,
    }

    /// The levels the player may choose from: the Y constant group `levels`.
    pub mod levels {
        use super::Level;

        /// From the easiest to the hardest. Each looks at ten times as many positions as the one before.
        pub const ALL: [Level; 5] = [
            Level { name: "Beginner", checks: 25, weight: 1 },
            Level { name: "Casual", checks: 250, weight: 2 },
            Level { name: "Club", checks: 2500, weight: 3 },
            Level { name: "Expert", checks: 25000, weight: 4 },
            Level { name: "Master", checks: 250000, weight: 5 },
        ];
    }

    /// Where a game stands, for the person playing.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Outcome {
        Playing,
        Won,
        Lost,
        Drawn,
    }

    /// One game.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Game {
        /// Every square, row by row from the top left: the piece on it, or none.
        pub(super) squares: Vec<Option<Piece>>,
        /// Whose turn it is.
        pub(super) to_move: Side,
        /// How hard the computer plays.
        pub(super) level: Level,
        /// How many moves have been made, by both players.
        pub(super) turns: u32,
        /// How many turns in a row have passed without a capture.
        pub(super) quiet: u32,
        /// The move made last, if one has been.
        pub(super) last: Option<Move>,
    }

    /// Starts a game: twelve men for each player on the dark squares of the three rows nearest them, and red to move.
    ///
    /// - `level`: how hard the computer plays.
    /// - Returns the game.
    pub fn new_game(level: Level) -> Game {
        let mut squares = Vec::new();
        for row in 0..board::SIZE { // y: #row
            for column in 0..board::SIZE { // y: #column
                squares.push(starting_piece(Square { row, column }));
            }
        }
        Game { squares, to_move: Side::Red, level, turns: 0, quiet: 0, last: None }
    }
}

/// The piece a square holds when a game starts.
///
/// - `square`: the square.
/// - Returns its piece, or none.
fn starting_piece(square: Square) -> Option<Piece> {
    if (square.row + square.column) % 2 == 0 { // y: #placed.light
        None
    } else if square.row < board::MEN_ROWS { // y: #placed.white_men
        Some(Piece { owner: Side::White, king: false })
    } else if square.row >= board::SIZE - board::MEN_ROWS { // y: #placed.red_men
        Some(Piece { owner: Side::Red, king: false })
    } else { // y: #placed.between
        None
    }
}

/// The Y facet `rules`: the moves a player may make, making them, and how a game ends and is scored.
pub mod rules {
    use super::*;

    /// The moves the player to move may make. A player who can capture must, and so may make only jumps.
    ///
    /// - `game`: the game.
    /// - Returns the moves.
    pub fn legal_moves(game: &Game) -> Vec<Move> {
        let jumps = all_jumps(game);
        if !jumps.is_empty() { // y: #must_jump
            jumps
        } else { // y: #no_jump
            all_steps(game)
        }
    }

    /// Makes a move: the piece leaves its square, the pieces it jumps over are taken off the board, and a man that
    /// ends on the far row is crowned. Then it is the other player's turn.
    ///
    /// - `game`: the game, which changes.
    /// - `chosen`: the move.
    pub fn apply(game: &mut Game, chosen: Move) {
        require(chosen.piece.owner == game.to_move, "checkers::apply#own_piece");
        let to = list::last(&chosen.path);
        let crowned = Piece { owner: chosen.piece.owner, king: chosen.piece.king || to.row == far_row(chosen.piece.owner) };
        list::set_number(&mut game.squares, number_of(chosen.from), None);
        let mut ran = false;
        for captured in &chosen.captures { // y: #captured
            ran = true;
            list::set_number(&mut game.squares, number_of(*captured), None);
            game.quiet = 0;
        }
        if !ran { // y: #quiet
            game.quiet += 1;
        }
        list::set_number(&mut game.squares, number_of(to), Some(crowned));
        game.turns += 1;
        game.to_move = other(game.to_move);
        game.last = Some(chosen);
    }

    /// Where a game stands, for the person playing red. A player whose turn it is and who cannot move has lost, and a
    /// game in which too many turns pass without a capture is drawn.
    ///
    /// - `game`: the game.
    /// - Returns where it stands.
    pub fn standing(game: &Game) -> Outcome {
        let stuck = legal_moves(game).is_empty();
        if stuck && game.to_move == Side::White { // y: #judged.won
            Outcome::Won
        } else if stuck { // y: #judged.lost
            Outcome::Lost
        } else if game.quiet >= game_rules::QUIET_LIMIT { // y: #judged.drawn
            Outcome::Drawn
        } else { // y: #judged.playing
            Outcome::Playing
        }
    }

    /// The score of a game: nothing unless the person won it. A win scores the win points, less some for each of
    /// the person's moves, but never less than the least win, and all of it times the level's weight.
    ///
    /// - `game`: the game.
    /// - Returns the score.
    pub fn score(game: &Game) -> u32 {
        if standing(game) == Outcome::Won { // y: #won
            let taken = (player_moves(game) * scoring::POINTS_PER_MOVE).min(scoring::WIN_POINTS - scoring::LEAST_WIN);
            (scoring::WIN_POINTS - taken) * game.level.weight
        } else { // y: #unfinished
            0
        }
    }

    /// How many moves the person has made. Red moves first, so red has made every odd turn.
    ///
    /// - `game`: the game.
    /// - Returns the count.
    pub fn player_moves(game: &Game) -> u32 {
        game.turns.div_ceil(2)
    }

    /// The player who moves after `player`.
    ///
    /// - `player`: a side.
    /// - Returns the other side.
    pub fn other(player: Side) -> Side {
        match player {
            Side::Red => Side::White, // y: #current.red
            Side::White => Side::Red, // y: #current.white
        }
    }
}

/// The steps the player to move may make: to an empty square next to a piece, the ways it may move.
///
/// - `game`: the game.
/// - Returns the steps.
fn all_steps(game: &Game) -> Vec<Move> {
    let mut found = Vec::new();
    for placed in own_pieces(game) { // y: #placed, else #no_pieces
        for direction in directions_of(placed.piece) { // y: #direction
            let to = Square { row: placed.square.row + direction.rows, column: placed.square.column + direction.columns };
            if on_board(to) && piece_at(game, to).is_none() { // y: #open
                found.push(Move { piece: placed.piece, from: placed.square, path: vec![to], captures: Vec::new() });
            } else { // y: #blocked
                continue;
            }
        }
    }
    found
}

/// The jumps the player to move may make, each as long a chain as it can go.
///
/// - `game`: the game.
/// - Returns the jumps.
fn all_jumps(game: &Game) -> Vec<Move> {
    let mut found = Vec::new();
    for placed in own_pieces(game) { // y: #placed, else #no_pieces
        found.extend(chains(game, placed.piece, placed.square, placed.square, &[], &[]));
    }
    found
}

/// The chains of jumps a piece can go on with from a square it has reached. A chain goes on for as long as the piece
/// can jump again, and ends when a man is crowned. A piece is not jumped twice, and the square the chain started from
/// counts as empty, as the piece has left it.
///
/// - `game`: the game, as it was before the chain started.
/// - `piece`: the piece that jumps.
/// - `start`: the square the chain started from.
/// - `at`: the square the piece has reached.
/// - `path`: the squares it has landed on so far.
/// - `captured`: the squares of the pieces it has jumped over so far.
/// - Returns the chains, each a whole move, or none when it cannot jump from `at`.
fn chains(game: &Game, piece: Piece, start: Square, at: Square, path: &[Square], captured: &[Square]) -> Vec<Move> {
    let mut found = Vec::new();
    for direction in directions_of(piece) { // y: #direction
        let over = Square { row: at.row + direction.rows, column: at.column + direction.columns };
        let land = Square { row: over.row + direction.rows, column: over.column + direction.columns };
        if jumpable(game, piece, start, over, land) && !captured.contains(&over) { // y: #can_jump
            let mut longer = path.to_vec(); // y: a copy
            longer.push(land);
            let mut taken = captured.to_vec(); // y: a copy
            taken.push(over);
            let further = chains_after(game, piece, start, land, &longer, &taken);
            if further.is_empty() { // y: #ends_here
                found.push(Move { piece, from: start, path: longer, captures: taken });
            } else { // y: #goes_on
                found.extend(further);
            }
        } else { // y: #cannot_jump
            continue;
        }
    }
    found
}

/// The chains of jumps that go on from a square a piece has just landed on: none when the landing crowns a man.
///
/// - `game`, `piece`, `start`, `land`, `path`, `captured`: as for `chains`, with `land` the square just landed on.
/// - Returns the chains.
fn chains_after(game: &Game, piece: Piece, start: Square, land: Square, path: &[Square], captured: &[Square]) -> Vec<Move> {
    if !piece.king && land.row == far_row(piece.owner) { // y: #crowned
        Vec::new()
    } else { // y: #uncrowned
        chains(game, piece, start, land, path, captured)
    }
}

/// Whether a piece may jump over one square onto the next: the square jumped over holds the other player's piece,
/// and the square landed on is on the board and empty.
///
/// - `game`: the game, as it was before the chain started.
/// - `piece`: the piece that jumps.
/// - `start`: the square the chain started from, which counts as empty.
/// - `over`: the square jumped over.
/// - `land`: the square landed on.
/// - Returns whether it may.
fn jumpable(game: &Game, piece: Piece, start: Square, over: Square, land: Square) -> bool {
    if on_board(land) { // y: #inside
        let free = piece_at(game, land).is_none() || land == start;
        belongs_to(game, over, other(piece.owner)) && free
    } else { // y: #outside
        false
    }
}

/// Whether a square holds a piece of a player's.
///
/// - `game`: the game.
/// - `square`: the square, on the board.
/// - `owner`: the player.
/// - Returns whether it does.
fn belongs_to(game: &Game, square: Square, owner: Side) -> bool {
    match piece_at(game, square) {
        None => false, // y: #held.empty
        Some(held) => held.owner == owner, // y: #held.piece
    }
}

/// Every piece on the board, with its square, row by row from the top left.
///
/// - `game`: the game.
/// - Returns the pieces.
fn placed_pieces(game: &Game) -> Vec<Placed> {
    let mut found = Vec::new();
    for row in 0..board::SIZE { // y: #row
        for column in 0..board::SIZE { // y: #column
            let here = Square { row, column };
            match piece_at(game, here) {
                None => continue, // y: #held.empty
                Some(held) => found.push(Placed { square: here, piece: held }), // y: #held.piece
            }
        }
    }
    found
}

/// The pieces of the player to move, with their squares.
///
/// - `game`: the game.
/// - Returns the pieces.
fn own_pieces(game: &Game) -> Vec<Placed> {
    let mut found = Vec::new();
    for placed in placed_pieces(game) { // y: #placed, else #no_pieces
        if placed.piece.owner == game.to_move { // y: #mine
            found.push(placed);
        } else { // y: #theirs
            continue;
        }
    }
    found
}

/// The ways a piece may move.
///
/// - `piece`: the piece.
/// - Returns the directions: two for a man, and four for a king.
fn directions_of(piece: Piece) -> &'static [Direction] {
    let ways: &'static [Direction] = if piece.king { // y: #moving.king
        &directions::ALL
    } else if piece.owner == Side::Red { // y: #moving.red
        &directions::UP
    } else { // y: #moving.white
        &directions::DOWN
    };
    ensure(list::count(ways) >= 2, "checkers::directions_of#several");
    ways
}

/// The row on which a player's men are crowned: the row farthest from where they start.
///
/// - `owner`: the player.
/// - Returns the row.
fn far_row(owner: Side) -> i32 {
    if owner == Side::Red { // y: #heading_up
        0
    } else { // y: #heading_down
        board::SIZE - 1
    }
}

/// The piece on a square, if there is one.
///
/// - `game`: the game.
/// - `square`: the square, on the board.
/// - Returns the piece, or none.
fn piece_at(game: &Game, square: Square) -> Option<Piece> {
    list::at_number(&game.squares, number_of(square))
}

/// The number of a square in a game's list of squares, counting from 1 row by row from the top left.
///
/// - `square`: the square, on the board.
/// - Returns the number.
fn number_of(square: Square) -> u32 {
    number::to_uint32(square.row * board::SIZE + square.column + 1)
}

/// Whether a square is on the board.
///
/// - `square`: the square.
/// - Returns whether it is.
fn on_board(square: Square) -> bool {
    let rows = square.row >= 0 && square.row < board::SIZE;
    let columns = square.column >= 0 && square.column < board::SIZE;
    rows && columns
}

/// The Y facet `computer`: how the computer chooses its moves.
pub mod computer {
    use super::*;

    /// What the computer's search counts: the Y constant group `search`.
    pub mod search {
        /// The most moves deep the minimax search goes.
        pub const DEEPEST: u32 = 10;
        /// The most moves a game played out at random runs for.
        pub const PLAYOUT_TURNS: u32 = 12;
        /// What a game won is worth, more than any count of pieces.
        pub const WIN_VALUE: i32 = 100;
        /// What a man is worth.
        pub const MAN_VALUE: i32 = 3;
        /// What a king is worth.
        pub const KING_VALUE: i32 = 5;
    }

    /// A game played out at random from a position.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct PlayedOut {
        /// What the position it ended in is worth.
        pub(super) value: i32,
        /// How many positions it looked at: one for each move made, and one for the position it ended in.
        pub(super) spent: u32,
    }

    /// Chooses the computer's move, looking at no more positions than its level allows: its checks.
    ///
    /// Half the checks go to a minimax search, which weighs each move by the pieces on the board when both players
    /// play their best for a number of moves ahead: one, then two, and so on, for as long as the checks last. The rest
    /// go to a Monte Carlo search among the moves the minimax search weighs best: each is worth the average of games
    /// played out at random from it.
    ///
    /// - `game`: the game.
    /// - `random`: the Y `injected` parameter.
    /// - Returns the move.
    pub fn computer_move(game: &Game, random: &mut Random) -> Move {
        require(!legal_moves(game).is_empty(), "checkers::computer_move#has_moves");
        let options = random.shuffled(&legal_moves(game));
        let allowance = game.level.checks / 2;
        let mut left = allowance;
        let mut values = list::repeated(0, list::count(&options));
        let mut depth = 1;
        let mut exhausted = false;
        while depth <= search::DEEPEST && !exhausted { // y: #deepening
            let mut found = Vec::new();
            for candidate in &options { // y: #candidate
                let mut after = game.clone(); // y: a copy
                apply(&mut after, candidate.clone()); // y: a copy
                found.push(searched(&after, game.to_move, depth - 1, &mut left));
            }
            if left > 0 { // y: #finished
                values = found;
                depth += 1;
            } else { // y: #ran_out
                exhausted = true;
            }
        }
        best_sampled(game, &options, &values, game.level.checks - (allowance - left), random)
    }
}

/// What a position is worth to a player when both players play their best for a number of moves ahead: the minimax
/// value. Each position it reaches is a check; when the checks run out, it gives up, leaves none, and its value is
/// not to be used.
///
/// - `game`: the position.
/// - `owner`: the player.
/// - `depth`: how many moves ahead to look.
/// - `left`: how many checks are left, which changes.
/// - Returns the value.
fn searched(game: &Game, owner: Side, depth: u32, left: &mut u32) -> i32 {
    let moves = legal_moves(game);
    if depth == 0 || moves.is_empty() || game.quiet >= game_rules::QUIET_LIMIT { // y: #judged.leaf
        value_for(game, owner)
    } else if list::count(&moves) >= *left { // y: #judged.spent
        *left = 0;
        0
    } else { // y: #judged.inner
        *left -= list::count(&moves);
        let mut values = Vec::new();
        for candidate in moves { // y: #candidate
            let mut after = game.clone(); // y: a copy
            apply(&mut after, candidate);
            values.push(searched(&after, owner, depth - 1, left));
        }
        if game.to_move == owner { // y: #owners_turn
            list::largest(&values)
        } else { // y: #others_turn
            list::smallest(&values)
        }
    }
}

/// The best of the moves the minimax search weighs best, by games played out at random from each within a share of
/// the checks. When one move is weighed best, it is chosen without them.
///
/// - `game`: the game.
/// - `options`: the moves.
/// - `values`: what the minimax search weighs each move at, in the same order.
/// - `checks`: how many positions the games may look at in all.
/// - `random`: the Y `injected` parameter.
/// - Returns the move.
fn best_sampled(game: &Game, options: &[Move], values: &[i32], checks: u32, random: &mut Random) -> Move {
    require(!options.is_empty(), "checkers::best_sampled#has_options");
    let best = list::largest(values);
    let mut tied = Vec::new();
    for option in 1..=list::count(options) { // y: #option
        match list::at_number(values, option).cmp(&best) {
            Ordering::Equal => tied.push(option), // y: #ranked.top
            Ordering::Less => continue, // y: #ranked.lower
            Ordering::Greater => not_reachable("checkers::best_sampled#ranked.above"), // y: #ranked.above
        }
    }
    match list::count(&tied).cmp(&1) {
        Ordering::Equal => { // y: #ties.settled
            list::at_number_ref(options, tied[0]).clone() // y: a copy
        }
        Ordering::Greater => { // y: #ties.contested
            let share = checks / list::count(&tied);
            let mut averages = Vec::new();
            for option in &tied { // y: #option
                let mut after = game.clone(); // y: a copy
                apply(&mut after, list::at_number_ref(options, *option).clone()); // y: a copy
                averages.push(sampled_value(&after, game.to_move, share, random));
            }
            list::at_number_ref(options, list::at_number(&tied, list::number_of(&averages, &list::largest(&averages)))).clone() // y: a copy
        }
        Ordering::Less => not_reachable("checkers::best_sampled#ties.none"), // y: #ties.none
    }
}

/// What a position is worth to a player, from games played out from it at random within a share of the checks; or,
/// when there is no share, from the pieces on the board.
///
/// - `position`: the position.
/// - `owner`: the player.
/// - `share`: how many positions the games may look at.
/// - `random`: the Y `injected` parameter.
/// - Returns the average of what the games' ends are worth, or the position's own worth.
fn sampled_value(position: &Game, owner: Side, share: u32, random: &mut Random) -> i32 {
    if share == 0 { // y: #no_share
        value_for(position, owner)
    } else { // y: #some_share
        let mut left = share;
        let mut total = 0;
        let mut played = 0;
        while left > 0 { // y: #sampling
            let length = (left - 1).min(search::PLAYOUT_TURNS);
            let outcome = playout(position, owner, length, random);
            total += outcome.value;
            played += 1;
            left -= outcome.spent;
        }
        total / played
    }
}

/// Plays a game out from a position, each move chosen at random, until it ends or `length` moves are made.
///
/// - `start`: the position.
/// - `owner`: the player whose worth the end is measured by.
/// - `length`: the most moves to make.
/// - `random`: the Y `injected` parameter.
/// - Returns what the end is worth, and how many positions were looked at.
fn playout(start: &Game, owner: Side, length: u32, random: &mut Random) -> PlayedOut {
    let mut position = start.clone(); // y: a copy
    let mut moves = legal_moves(&position);
    let mut made = 0;
    while made < length && !moves.is_empty() && position.quiet < game_rules::QUIET_LIMIT { // y: #going, else #ended
        apply(&mut position, random.one_of(&moves));
        moves = legal_moves(&position);
        made += 1;
    }
    PlayedOut { value: value_for(&position, owner), spent: made + 1 }
}

/// What a position is worth to a player: the win value for a win, less it for a loss, nothing for a draw, and
/// otherwise what the player's pieces are worth less what the other player's are.
///
/// - `game`: the position.
/// - `owner`: the player.
/// - Returns the worth.
fn value_for(game: &Game, owner: Side) -> i32 {
    let stuck = legal_moves(game).is_empty();
    if stuck && game.to_move == owner { // y: #judged.losing
        -search::WIN_VALUE
    } else if stuck { // y: #judged.winning
        search::WIN_VALUE
    } else if game.quiet >= game_rules::QUIET_LIMIT { // y: #judged.drawn
        0
    } else { // y: #judged.playing
        material(game, owner) - material(game, other(owner))
    }
}

/// What a player's pieces on the board are worth.
///
/// - `game`: the game.
/// - `owner`: the player.
/// - Returns the worth.
fn material(game: &Game, owner: Side) -> i32 {
    let mut total = 0;
    for placed in placed_pieces(game) { // y: #placed, else #no_pieces
        if placed.piece.owner != owner { // y: #counted.other
            continue;
        } else if placed.piece.king { // y: #counted.king
            total += search::KING_VALUE;
        } else { // y: #counted.man
            total += search::MAN_VALUE;
        }
    }
    total
}

/// The Y facet `drawing`: what the player sees.
pub mod drawing {
    use super::*;

    /// The sizes of what is drawn, in pixels: the Y constant group `layout`.
    pub mod layout {
        use super::board;

        /// The side of a square.
        pub const SQUARE_SIZE: i32 = 72;
        /// The space around the board.
        pub const GAP: i32 = 12;
        /// The side of the board.
        pub const BOARD_SIZE: i32 = board::SIZE * SQUARE_SIZE;
        /// The radius of a piece, of the ring inside it, and of the mark of a square a piece may move to.
        pub const PIECE_RADIUS: i32 = 28;
        pub const RING_RADIUS: i32 = 22;
        pub const CENTRE_RADIUS: i32 = 12;
        pub const MARK_RADIUS: i32 = 10;
        /// The height of the bar beneath the board, where the score is written.
        pub const BAR_HEIGHT: i32 = 84;
        /// The height of the text, and of a title.
        pub const TEXT_SIZE: u32 = 20;
        pub const TITLE_SIZE: u32 = 40;
        /// Where the first button of the levels is, how high each is, and how far it is from one to the next.
        pub const BUTTON_TOP: i32 = 140;
        pub const BUTTON_HEIGHT: i32 = 72;
        pub const BUTTON_STEP: i32 = 88;
        pub const WIDTH: i32 = GAP + BOARD_SIZE + GAP;
        pub const HEIGHT: i32 = GAP + BOARD_SIZE + BAR_HEIGHT;
    }

    /// The colours of what is drawn: the Y constant group `palette`.
    pub mod palette {
        use super::Colour;

        pub const BACKGROUND: Colour = Colour::rgb(30, 28, 26);
        pub const LIGHT: Colour = Colour::rgb(232, 214, 178);
        pub const DARK: Colour = Colour::rgb(118, 82, 56);
        pub const SELECTED: Colour = Colour::rgb(86, 136, 84);
        pub const LAST: Colour = Colour::rgb(150, 112, 60);
        pub const MARK: Colour = Colour::rgb(140, 210, 130);
        pub const RED: Colour = Colour::rgb(196, 42, 38);
        pub const RED_RING: Colour = Colour::rgb(150, 28, 26);
        pub const WHITE: Colour = Colour::rgb(240, 236, 224);
        pub const WHITE_RING: Colour = Colour::rgb(196, 190, 176);
        pub const CROWN: Colour = Colour::rgb(232, 190, 60);
        pub const BUTTON: Colour = Colour::rgb(70, 64, 58);
        pub const TEXT: Colour = Colour::rgb(232, 230, 226);
        pub const MUTED: Colour = Colour::rgb(170, 166, 160);
    }

    /// The lines written beneath the board: how the game stands, and what the player can do.
    ///
    /// - `game`: the game.
    /// - `highest`: the best score so far.
    /// - Returns the two lines.
    pub fn banner(game: &Game, highest: u32) -> Vec<String> {
        let moves = player_moves(game);
        let name = game.level.name;
        match standing(game) {
            Outcome::Playing if game.to_move == Side::Red => vec![ // y: #shown.your_move
                format!("Your move   {name}   Moves {moves}   Best {highest}"),
                "Click a red piece, then a marked square. Escape quits.".to_string(),
            ],
            Outcome::Playing => vec![ // y: #shown.thinking
                format!("The computer is thinking, at {} positions a move...", game.level.checks),
                "Escape quits.".to_string(),
            ],
            Outcome::Won => vec![ // y: #shown.won
                format!("You won in {moves} moves against {name}! Score {}   Best {highest}", score(game)),
                "Space chooses a new game. Escape quits.".to_string(),
            ],
            Outcome::Lost => vec![ // y: #shown.lost
                format!("The computer won. Score 0   Best {highest}"),
                "Space chooses a new game. Escape quits.".to_string(),
            ],
            Outcome::Drawn => vec![ // y: #shown.drawn
                format!("A draw: {} turns without a capture. Score 0   Best {highest}", game_rules::QUIET_LIMIT),
                "Space chooses a new game. Escape quits.".to_string(),
            ],
        }
    }

    /// Draws a game: the board, the pieces, the squares the selected piece may move to, and the banner beneath.
    ///
    /// - `game`: the game.
    /// - `selected`: the square of the piece the player has selected, if there is one.
    /// - `highest`: the best score so far.
    /// - `window`: the Y `injected` parameter.
    pub fn draw(game: &Game, selected: Option<Square>, highest: u32, window: &mut Window) {
        window.clear(palette::BACKGROUND);
        draw_board(game, selected, window);
        draw_pieces(game, window);
        draw_marks(game, selected, window);
        let lines = banner(game, highest);
        let top = layout::HEIGHT - layout::BAR_HEIGHT;
        window.write(&lines[0], layout::GAP, top + 32, layout::TEXT_SIZE, palette::TEXT);
        window.write(&lines[1], layout::GAP, top + 60, layout::TEXT_SIZE, palette::MUTED);
    }

    /// Draws the levels to choose from, each a button, and the best score so far.
    ///
    /// - `highest`: the best score so far.
    /// - `window`: the Y `injected` parameter.
    pub fn draw_levels(highest: u32, window: &mut Window) {
        window.clear(palette::BACKGROUND);
        window.write("Checkers", layout::GAP, 60, layout::TITLE_SIZE, palette::TEXT);
        let prompt = "Click how hard the computer plays. Escape quits.";
        window.write(prompt, layout::GAP, 100, layout::TEXT_SIZE, palette::MUTED);
        let mut top = layout::BUTTON_TOP;
        for level in levels::ALL { // y: #level
            let checks = format!("Looks at {} positions a move. Wins score x{}.", level.checks, level.weight);
            window.fill(layout::GAP, top, layout::BOARD_SIZE, layout::BUTTON_HEIGHT, palette::BUTTON);
            window.write(level.name, layout::GAP + 16, top + 30, layout::TEXT_SIZE, palette::TEXT);
            window.write(&checks, layout::GAP + 16, top + 56, layout::TEXT_SIZE, palette::MUTED);
            top += layout::BUTTON_STEP;
        }
        window.write(&format!("Best {highest}"), layout::GAP, top + 24, layout::TEXT_SIZE, palette::TEXT);
    }

    /// The square under a point, if there is one.
    ///
    /// - `place`: the point.
    /// - Returns the square, or none when the point is off the board.
    pub fn square_at(place: Point) -> Option<Square> {
        let across = place.x >= layout::GAP && place.x < layout::GAP + layout::BOARD_SIZE;
        let down = place.y >= layout::GAP && place.y < layout::GAP + layout::BOARD_SIZE;
        if across && down { // y: #inside
            let row = (place.y - layout::GAP) / layout::SQUARE_SIZE;
            let column = (place.x - layout::GAP) / layout::SQUARE_SIZE;
            Some(Square { row, column })
        } else { // y: #outside
            None
        }
    }

    /// The level whose button is under a point, if there is one.
    ///
    /// - `place`: the point.
    /// - Returns the level, or none when the point is on no button.
    pub fn level_at(place: Point) -> Option<Level> {
        let mut top = layout::BUTTON_TOP;
        for level in levels::ALL { // y: #level
            let across = place.x >= layout::GAP && place.x < layout::GAP + layout::BOARD_SIZE;
            let down = place.y >= top && place.y < top + layout::BUTTON_HEIGHT;
            if across && down { // y: #pressed
                return Some(level);
            } else { // y: #missed
                top += layout::BUTTON_STEP;
            }
        }
        None
    }
}

/// The top left corner of a square.
///
/// - `square`: the square.
/// - Returns the corner.
fn corner_of(square: Square) -> Point {
    Point { x: layout::GAP + square.column * layout::SQUARE_SIZE, y: layout::GAP + square.row * layout::SQUARE_SIZE }
}

/// The centre of a square.
///
/// - `square`: the square.
/// - Returns the centre.
fn centre_of(square: Square) -> Point {
    let corner = corner_of(square);
    Point { x: corner.x + layout::SQUARE_SIZE / 2, y: corner.y + layout::SQUARE_SIZE / 2 }
}

/// The colour of a square: light or dark, or marked as the selected piece's or as part of the last move.
///
/// - `game`: the game.
/// - `square`: the square.
/// - `selected`: the square of the piece the player has selected, if there is one.
/// - Returns the colour.
fn shade_of(game: &Game, square: Square, selected: Option<Square>) -> Colour {
    if (square.row + square.column) % 2 == 0 { // y: #looks.light
        palette::LIGHT
    } else if selected == Some(square) { // y: #looks.selected
        palette::SELECTED
    } else if in_last_move(game, square) { // y: #looks.last
        palette::LAST
    } else { // y: #looks.dark
        palette::DARK
    }
}

/// Whether a square is one the last move started from or landed on.
///
/// - `game`: the game.
/// - `square`: the square.
/// - Returns whether it is.
fn in_last_move(game: &Game, square: Square) -> bool {
    match &game.last {
        None => false, // y: #made.none
        Some(made) => made.from == square || made.path.contains(&square), // y: #made.some
    }
}

/// The squares the selected piece may move to.
///
/// - `game`: the game.
/// - `selected`: the square of the piece the player has selected, if there is one.
/// - Returns the squares: none when no piece is selected.
fn landings(game: &Game, selected: Option<Square>) -> Vec<Square> {
    let mut found = Vec::new();
    for candidate in legal_moves(game) { // y: #candidate, else #no_moves
        if Some(candidate.from) == selected { // y: #from_selected
            found.push(list::last(&candidate.path));
        } else { // y: #from_elsewhere
            continue;
        }
    }
    found
}

/// Draws the squares of the board.
///
/// - `game`: the game.
/// - `selected`: the square of the piece the player has selected, if there is one.
/// - `window`: the Y `injected` parameter.
fn draw_board(game: &Game, selected: Option<Square>, window: &mut Window) {
    for row in 0..board::SIZE { // y: #row
        for column in 0..board::SIZE { // y: #column
            let here = Square { row, column };
            let corner = corner_of(here);
            let shade = shade_of(game, here, selected);
            window.fill(corner.x, corner.y, layout::SQUARE_SIZE, layout::SQUARE_SIZE, shade);
        }
    }
}

/// Draws every piece on the board.
///
/// - `game`: the game.
/// - `window`: the Y `injected` parameter.
fn draw_pieces(game: &Game, window: &mut Window) {
    for placed in placed_pieces(game) { // y: #placed, else #no_pieces
        draw_piece(placed, window);
    }
}

/// Marks the squares the selected piece may move to.
///
/// - `game`: the game.
/// - `selected`: the square of the piece the player has selected, if there is one.
/// - `window`: the Y `injected` parameter.
fn draw_marks(game: &Game, selected: Option<Square>, window: &mut Window) {
    for landing in landings(game, selected) { // y: #landing, else #no_marks
        let centre = centre_of(landing);
        window.circle(centre.x, centre.y, layout::MARK_RADIUS, palette::MARK);
    }
}

/// Draws one piece: a disc in its player's colours, with a ring inside, and a crown at its centre if it is a king.
///
/// - `placed`: the piece and its square.
/// - `window`: the Y `injected` parameter.
fn draw_piece(placed: Placed, window: &mut Window) {
    let centre = centre_of(placed.square);
    let colours = colours_of(placed.piece);
    window.circle(centre.x, centre.y, layout::PIECE_RADIUS, colours[0]);
    window.circle(centre.x, centre.y, layout::RING_RADIUS, colours[1]);
    window.circle(centre.x, centre.y, layout::CENTRE_RADIUS, colours[2]);
}

/// The colours a piece is drawn in.
///
/// - `piece`: the piece.
/// - Returns the colours of its disc, its ring and its centre.
fn colours_of(piece: Piece) -> Vec<Colour> {
    if piece.owner == Side::Red && piece.king { // y: #painted.red_king
        vec![palette::RED, palette::RED_RING, palette::CROWN]
    } else if piece.owner == Side::Red { // y: #painted.red_man
        vec![palette::RED, palette::RED_RING, palette::RED]
    } else if piece.king { // y: #painted.white_king
        vec![palette::WHITE, palette::WHITE_RING, palette::CROWN]
    } else { // y: #painted.white_man
        vec![palette::WHITE, palette::WHITE_RING, palette::WHITE]
    }
}

/// The Y facet `window_play`: playing in a window, one game after another, until the window is closed.
pub mod window_play {
    use super::*;

    /// How a game played in the window ended.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Ending {
        /// The game's score.
        pub(super) points: u32,
        /// Whether the player asked to stop playing.
        pub(super) quitting: bool,
    }

    /// Plays Checkers in a window: the player chooses a level, then plays a game against the computer, and so on,
    /// until the player closes the window or presses Escape.
    ///
    /// - `window`, `random`, `forever`: the Y `injected` parameters.
    /// - Returns the best score of the games played.
    pub async fn play(window: &mut Window, random: &mut Random, forever: &Forever) -> u32 {
        let mut highest = 0;
        let mut quitting = false;
        while !quitting { // y: #open
            match choose_level(highest, window, forever).await {
                None => quitting = true, // y: #picked.none
                Some(picked) => { // y: #picked.level, which narrows `picked` to a `level`
                    let ended = play_game(new_game(picked), highest, window, random, forever).await;
                    highest = highest.max(ended.points);
                    quitting = ended.quitting;
                }
            }
        }
        highest
    }

    /// Shows the levels until the player clicks one, presses Escape or closes the window.
    ///
    /// - `highest`: the best score so far.
    /// - `window`, `forever`: the Y `injected` parameters.
    /// - Returns the level clicked, or none when the player stopped.
    pub async fn choose_level(highest: u32, window: &mut Window, forever: &Forever) -> Option<Level> {
        let mut chosen = None;
        let mut quitting = false;
        while chosen.is_none() && !quitting && !window.close_requested() { // y: #choosing, else #closed_at_once
            quitting = window.keys_pressed().contains(&Key::Escape);
            chosen = level_clicked(&window.clicks());
            draw_levels(highest, window);
            window.next_frame(forever).await; // y: forever because "a hidden window shows no frames, and the game waits until it is shown"
        }
        chosen
    }

    /// Plays a game, until it ends and the player presses Space, or the player presses Escape or closes the window.
    ///
    /// - `started`: the game as it starts.
    /// - `highest`: the best score so far.
    /// - `window`, `random`, `forever`: the Y `injected` parameters.
    /// - Returns how the game ended.
    pub async fn play_game(started: Game, highest: u32, window: &mut Window, random: &mut Random, forever: &Forever) -> Ending {
        let mut game = started;
        let mut selected = None;
        let mut quitting = false;
        let mut again = false;
        while !quitting && !again && !window.close_requested() { // y: #playing, else #closed_at_once
            let keys = window.keys_pressed();
            quitting = keys.contains(&Key::Escape);
            if standing(&game) != Outcome::Playing { // y: #moment.over
                again = keys.contains(&Key::Space);
            } else if game.to_move == Side::White { // y: #moment.computers_turn
                let chosen = computer_move(&game, random);
                apply(&mut game, chosen);
            } else { // y: #moment.players_turn
                act_on_clicks(&mut game, &mut selected, &window.clicks());
            }
            draw(&game, selected, highest, window);
            window.next_frame(forever).await; // y: forever because "a hidden window shows no frames, and the game waits until it is shown"
        }
        Ending { points: score(&game), quitting }
    }

    /// Acts on a click of the player's: a click on a square the selected piece may move to makes that move, a click on
    /// a piece of the player's that may move selects it, and any other click selects nothing.
    ///
    /// - `game`: the game, which changes.
    /// - `selected`: the square of the piece the player has selected, if there is one, which changes.
    /// - `place`: the point clicked.
    pub fn clicked(game: &mut Game, selected: &mut Option<Square>, place: Point) {
        let target = square_at(place);
        let found = move_to(game, *selected, target);
        if let Some(found) = found { // y: #judged.moving, which narrows `found` to a `move`
            apply(game, found);
            *selected = None;
        } else if may_move_from(game, target) { // y: #judged.choosing
            *selected = target;
        } else { // y: #judged.elsewhere
            *selected = None;
        }
    }
}

/// The level whose button the player clicked first, if any click was on one.
///
/// - `clicks`: the points clicked, in order.
/// - Returns the level, or none.
fn level_clicked(clicks: &[Point]) -> Option<Level> {
    for place in clicks { // y: #place, else #no_clicks
        let picked = level_at(*place);
        if picked.is_some() { // y: #hit
            return picked;
        } else { // y: #miss
            continue;
        }
    }
    None
}

/// Acts on each of the player's clicks in turn.
///
/// - `game`: the game, which changes.
/// - `selected`: the square of the piece the player has selected, if there is one, which changes.
/// - `clicks`: the points clicked, in order.
fn act_on_clicks(game: &mut Game, selected: &mut Option<Square>, clicks: &[Point]) {
    for place in clicks { // y: #place, else #no_clicks
        clicked(game, selected, *place);
    }
}

/// The move from one square to another, if the player to move may make it.
///
/// - `game`: the game.
/// - `from`: the square the move starts from, if there is one.
/// - `to`: the square it ends on, if there is one.
/// - Returns the move, or none.
fn move_to(game: &Game, from: Option<Square>, to: Option<Square>) -> Option<Move> {
    for candidate in legal_moves(game) { // y: #candidate, else #no_moves
        if Some(candidate.from) == from && Some(list::last(&candidate.path)) == to { // y: #fits
            return Some(candidate);
        } else { // y: #differs
            continue;
        }
    }
    None
}

/// Whether the player to move may move the piece on a square.
///
/// - `game`: the game.
/// - `square`: the square, if there is one.
/// - Returns whether they may.
fn may_move_from(game: &Game, square: Option<Square>) -> bool {
    for candidate in legal_moves(game) { // y: #candidate, else #no_moves
        if Some(candidate.from) == square { // y: #movable
            return true;
        } else { // y: #unmovable
            continue;
        }
    }
    false
}
