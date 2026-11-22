//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use checkers_example::checkers::drawing::layout;
use checkers_example::checkers::window_play::play;
use y_runtime::{ConsoleError, System};
use y_window::SystemWindow;

/// Macroquad opens the window before any code runs, so the window's title and size, which are constants in
/// `system.window(…)`, open it here, and the entry point runs on Macroquad's executor.
fn main() {
    y_window::run("Checkers", layout::WIDTH, layout::HEIGHT, entry());
}

/// Plays Checkers in a window, and when the window is closed, writes the best score at the terminal.
///
/// - Returns nothing.
/// - Fails: the terminal could not be written.
async fn entry() {
    let system = System::root();
    let console = system.console();
    {
        // y: inject(window = system.window(…), random = system.random(), forever = system.forever())
        let window = system.window();
        let random = system.random();
        let forever = system.forever();
        let best = play(&mut window, &mut random, &forever).await;
        console.write_line(&format!("Your best score: {best}."))?;
    }
    Ok(())
}
