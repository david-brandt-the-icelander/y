//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use tic_tac_toe_example::tic_tac_toe::console_play::{play, PlayError};
use tic_tac_toe_example::tic_tac_toe::setup::new_game;
use y_runtime::System;

/// Plays one game of tic-tac-toe at the terminal.
///
/// - Returns nothing. The game shows the player how it ended.
/// - Fails `InputClosed`: the terminal closed before the game ended.
/// - Fails `Console`: the terminal could not be read or written.
fn main() -> Result<(), PlayError> {
    let system = System::root();
    {
        // y: inject(console = system.console(), forever = system.forever())
        let mut console = system.console();
        let forever = system.forever();
        play(new_game(), &mut console, &forever)?;
    }
    Ok(())
}
