//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use nim_example::nim::console_play::{play, PlayError};
use y_runtime::System;

/// Plays one game of Nim at the terminal.
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
        play(&mut console, &forever)?;
    }
    Ok(())
}
