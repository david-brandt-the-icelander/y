//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use snake_example::snake::drawing::layout;
use snake_example::snake::window_play::play;
use y_runtime::{ConsoleError, System};
use y_window::SystemWindow;

/// Macroquad opens the window before any code runs, so the window's title and size, which are constants in
/// `system.window(…)`, open it here, and the entry point runs on Macroquad's executor.
fn main() {
    y_window::run("Snake", layout::WIDTH, layout::HEIGHT, entry());
}

/// Plays Snake in a window, and when the window is closed, writes the best score at the terminal.
///
/// - Returns nothing.
/// - Fails with a `ConsoleError`: the terminal could not be written.
async fn entry() {
    let system = System::root();
    let console = system.console();
    {
        // y: inject(window = system.window(…), clock = system.clock(), random = system.random(), forever = system.forever())
        let window = system.window();
        let clock = system.clock();
        let random = system.random();
        let forever = system.forever();
        let best = play(&mut window, &mut clock, &mut random, &forever).await;
        console.write_line(&format!("Your best score: {best}."))?;
    }
    Ok(())
}
