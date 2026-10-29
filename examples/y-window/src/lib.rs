//! A small stand-in for Y's `standard.window`, written by hand: the `window` capability, its `key` and `colour`
//! types, and the entry point that opens the window, drawn with Macroquad.
//!
//! It is a crate of its own, apart from `y-runtime`, so that the console examples do not build Macroquad. It is only
//! as large as the examples need.
//!
//! Y has no `async` (Y § 3.7): a function may suspend when a capability it holds may suspend, and `window.next_frame()`
//! does. Rust colours such a function, so the lowering makes it an `async fn` and awaits it, and the entry point runs
//! on Macroquad's executor. A fake window never suspends, and `block_on` runs a function that holds one in a test.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use y_runtime::{Forever, System};

/// A key the player can press: the ones the examples read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    Space,
    Escape,
}

impl Key {
    const ALL: [(Key, macroquad::input::KeyCode); 10] = {
        use macroquad::input::KeyCode;
        [
            (Key::Up, KeyCode::Up),
            (Key::Down, KeyCode::Down),
            (Key::Left, KeyCode::Left),
            (Key::Right, KeyCode::Right),
            (Key::W, KeyCode::W),
            (Key::A, KeyCode::A),
            (Key::S, KeyCode::S),
            (Key::D, KeyCode::D),
            (Key::Space, KeyCode::Space),
            (Key::Escape, KeyCode::Escape),
        ]
    };
}

/// A colour, as Y's `colour(red: …, green: …, blue: …)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Colour {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Colour {
        Colour { red, green, blue }
    }

    fn drawn(self) -> macroquad::color::Color {
        macroquad::color::Color::from_rgba(self.red, self.green, self.blue, 255)
    }
}

/// A fake window's script and what it has been given to draw.
struct Fake {
    /// The keys pressed in each frame, in order. Once they run out, the player has asked to close the window.
    frames: Vec<Vec<Key>>,
    /// How many frames have been shown.
    shown: usize,
    /// The text written, one entry per call.
    written: Vec<String>,
}

/// A window on the screen, or a fake of it for tests: it draws, and reads the keys pressed.
pub struct Window {
    fake: Option<Fake>,
}

/// `system.window()`: the window, which the entry point opened (see `run`).
pub trait SystemWindow {
    fn window(&self) -> Window;
}

impl SystemWindow for System {
    fn window(&self) -> Window {
        // closing the window is then something the program sees, and decides about
        macroquad::input::prevent_quit();
        Window { fake: None }
    }
}

impl Window {
    /// A fake window for a test: in each frame, the player presses the keys given for it; after the last frame, the
    /// player asks to close the window. It records the text written.
    pub fn fake(frames: &[&[Key]]) -> Window {
        let frames = frames.iter().map(|keys| keys.to_vec()).collect();
        Window { fake: Some(Fake { frames, shown: 0, written: Vec::new() }) }
    }

    /// Fills the whole window with a colour.
    pub fn clear(&mut self, colour: Colour) {
        if self.fake.is_none() {
            macroquad::window::clear_background(colour.drawn());
        }
    }

    /// Fills a rectangle, in pixels from the top left.
    pub fn fill(&mut self, x: i32, y: i32, width: i32, height: i32, colour: Colour) {
        if self.fake.is_none() {
            macroquad::shapes::draw_rectangle(x as f32, y as f32, width as f32, height as f32, colour.drawn());
        }
    }

    /// Writes a line of text whose baseline starts at `x`, `y`, in pixels from the top left, `size` pixels high.
    pub fn write(&mut self, text: &str, x: i32, y: i32, size: u32, colour: Colour) {
        match &mut self.fake {
            None => {
                macroquad::text::draw_text(text, x as f32, y as f32, size as f32, colour.drawn());
            }
            Some(fake) => fake.written.push(text.to_string()),
        }
    }

    /// The keys pressed since the last frame.
    pub fn keys_pressed(&self) -> Vec<Key> {
        match &self.fake {
            None => {
                let pressed = macroquad::input::get_keys_pressed();
                Key::ALL.iter().filter(|(_, code)| pressed.contains(code)).map(|(key, _)| *key).collect()
            }
            Some(fake) => fake.frames.get(fake.shown).cloned().unwrap_or_default(),
        }
    }

    /// Whether the player has asked to close the window.
    pub fn close_requested(&self) -> bool {
        match &self.fake {
            None => macroquad::input::is_quit_requested(),
            Some(fake) => fake.shown >= fake.frames.len(),
        }
    }

    /// Shows what has been drawn, and waits for the next frame. A hidden window shows no frames, so the wait has no
    /// bound, and needs `Forever`: the claim `forever because "..."` in Y.
    pub async fn next_frame(&mut self, _forever: &Forever) {
        match &mut self.fake {
            None => macroquad::window::next_frame().await,
            Some(fake) => fake.shown += 1,
        }
    }

    /// How many frames a fake window has shown.
    pub fn frames_shown(&self) -> u32 {
        self.fake.as_ref().map_or(0, |fake| fake.shown as u32)
    }

    /// The text a fake window has been given to write, one entry per call. Empty for a window on the screen.
    pub fn written(&self) -> &[String] {
        self.fake.as_ref().map_or(&[], |fake| &fake.written)
    }
}

/// Opens a window with a title and a size in pixels, then runs the entry point on Macroquad's executor. A failure
/// of the entry point ends the program as Rust's `main` does: the error is printed, and the exit status is 1.
pub fn run<E: std::fmt::Debug>(title: &str, width: i32, height: i32, entry: impl Future<Output = Result<(), E>> + 'static) {
    let conf = macroquad::window::Conf {
        window_title: title.to_string(),
        window_width: width,
        window_height: height,
        window_resizable: false,
        ..Default::default()
    };
    macroquad::Window::from_config(conf, async {
        if let Err(error) = entry.await {
            eprintln!("Error: {error:?}");
            std::process::exit(1);
        }
    });
}

/// Runs a function that holds only fake windows, which never suspend, to its end: for tests.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}
