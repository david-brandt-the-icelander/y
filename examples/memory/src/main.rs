//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use memory_example::memory::drawing::layout;
use memory_example::memory::setup::{openmoji, SetupError};
use memory_example::memory::window_play::play;
use y_http::SystemHttp;
use y_runtime::{ConsoleError, System};
use y_window::SystemWindow;

/// Why the program ended early: the Y type `main_error`.
#[derive(Clone, Debug, PartialEq)]
enum MainError {
    Setup { error: SetupError },
    Console { error: ConsoleError },
}

/// Y's `or fail` on a `SetupError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From<SetupError> for MainError {
    fn from(error: SetupError) -> MainError {
        MainError::Setup { error }
    }
}

/// Y's `or fail` on a `ConsoleError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From<ConsoleError> for MainError {
    fn from(error: ConsoleError) -> MainError {
        MainError::Console { error }
    }
}

/// Macroquad opens the window before any code runs, so the window's title and size, which are constants in
/// `system.window(…)`, open it here, and the entry point runs on Macroquad's executor.
fn main() {
    y_window::run("Memory", layout::WIDTH, layout::HEIGHT, entry());
}

/// Plays Memory in a window, and when the window is closed, writes the best score at the terminal.
///
/// - Returns nothing.
/// - Fails `Setup`: a game could not be set up, because its pictures could not be fetched.
/// - Fails `Console`: the terminal could not be written.
async fn entry() -> Result<(), MainError> {
    let system = System::root();
    let mut console = system.console();
    {
        // y: inject(window = system.window(…), clock = system.clock(), random = system.random(), site = system.http(openmoji), forever = system.forever())
        let mut window = system.window();
        let mut clock = system.clock();
        let mut random = system.random();
        let mut site = system.http(openmoji::ADDRESS);
        let forever = system.forever();
        let best = play(&mut window, &mut clock, &mut random, &mut site, &forever).await?;
        console.write_line(&format!("Your best score: {best}."))?;
    }
    Ok(())
}
