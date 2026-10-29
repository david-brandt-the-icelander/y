//! A small stand-in for Y's `standard.window`, written by hand: the `window` capability, its `key`, `colour`, `point`
//! and `picture` types, and the entry point that opens the window, drawn with Macroquad.
//!
//! It is a crate of its own, apart from `y-runtime`, so that the console examples do not build Macroquad. It is only
//! as large as the examples need.
//!
//! Y has no `async` (Y § 3.7): a function may suspend when a capability it holds may suspend, and `window.next_frame()`
//! does. Rust colours such a function, so the lowering makes it an `async fn` and awaits it, and the entry point runs
//! on Macroquad's executor. A fake window never suspends, and `block_on` runs a function that holds one in a test.

use std::cell::OnceCell;
use std::future::Future;
use std::pin::pin;
use std::rc::Rc;
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

/// A point in the window, in pixels from the top left, as Y's `point(x: …, y: …)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Why a text is not a picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureError {
    /// What is wrong with it, for a person to read.
    pub message: String,
}

/// A square picture, as Y's `picture`: its pixels, drawn from an SVG drawing at a size. It is plain data, so a test
/// can make one; the window turns it into a texture the first time it is drawn.
#[derive(Clone)]
pub struct Picture {
    size: u16,
    /// Red, green, blue and alpha for each pixel, row by row.
    pixels: Rc<[u8]>,
    texture: Rc<OnceCell<macroquad::texture::Texture2D>>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({}x{})", self.size, self.size)
    }
}

impl PartialEq for Picture {
    fn eq(&self, other: &Picture) -> bool {
        self.size == other.size && self.pixels == other.pixels
    }
}

impl Picture {
    /// The picture an SVG drawing makes when it is drawn `size` pixels square, its longer side filling the square:
    /// Y's `picture.from_svg(drawing, size: …)`.
    pub fn from_svg(drawing: &str, size: u16) -> Result<Picture, PictureError> {
        use resvg::{tiny_skia, usvg};
        let tree = usvg::Tree::from_str(drawing, &usvg::Options::default()).map_err(|e| PictureError { message: e.to_string() })?;
        let mut pixmap = tiny_skia::Pixmap::new(size.into(), size.into()).ok_or(PictureError { message: "a picture of no pixels".to_string() })?;
        let scale = f32::from(size) / tree.size().width().max(tree.size().height());
        resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        // tiny-skia keeps colours multiplied by alpha; a texture wants them apart
        let pixels: Vec<u8> = pixmap.pixels().iter().flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        }).collect();
        Ok(Picture { size, pixels: pixels.into(), texture: Rc::new(OnceCell::new()) })
    }

    /// The side of the picture, in pixels.
    pub fn size(&self) -> u16 {
        self.size
    }
}

/// What the player does in one frame of a fake window.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// The keys pressed.
    pub keys: Vec<Key>,
    /// The points clicked, in order.
    pub clicks: Vec<Point>,
}

/// A fake window's script and what it has been given to draw.
struct Fake {
    /// What the player does in each frame, in order. Once the frames run out, the player has asked to close the window.
    frames: Vec<Frame>,
    /// How many frames have been shown.
    shown: usize,
    /// The text written, one entry per call.
    written: Vec<String>,
    /// How many pictures have been drawn.
    pictures: u32,
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
        Window::fake_frames(frames.iter().map(|keys| Frame { keys: keys.to_vec(), clicks: Vec::new() }).collect())
    }

    /// A fake window for a test, in whose frames the player presses keys and clicks: Y's `fake window(frames: …)` with
    /// `frame(keys: …, clicks: …)`. After the last frame, the player asks to close the window.
    pub fn fake_frames(frames: Vec<Frame>) -> Window {
        Window { fake: Some(Fake { frames, shown: 0, written: Vec::new(), pictures: 0 }) }
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

    /// Draws a picture with its top left corner at `x`, `y`, in pixels from the top left, at its own size.
    pub fn picture(&mut self, picture: &Picture, x: i32, y: i32) {
        match &mut self.fake {
            None => {
                let texture = picture.texture.get_or_init(|| {
                    macroquad::texture::Texture2D::from_rgba8(picture.size, picture.size, &picture.pixels)
                });
                macroquad::texture::draw_texture(texture, x as f32, y as f32, macroquad::color::WHITE);
            }
            Some(fake) => fake.pictures += 1,
        }
    }

    /// The points clicked with the mouse since the last frame: at most one in a window on the screen.
    pub fn clicks(&self) -> Vec<Point> {
        match &self.fake {
            None => {
                let clicked = macroquad::input::is_mouse_button_pressed(macroquad::input::MouseButton::Left);
                let (x, y) = macroquad::input::mouse_position();
                if clicked { vec![Point { x: x as i32, y: y as i32 }] } else { Vec::new() }
            }
            Some(fake) => fake.frames.get(fake.shown).map(|frame| frame.clicks.clone()).unwrap_or_default(),
        }
    }

    /// The keys pressed since the last frame.
    pub fn keys_pressed(&self) -> Vec<Key> {
        match &self.fake {
            None => {
                let pressed = macroquad::input::get_keys_pressed();
                Key::ALL.iter().filter(|(_, code)| pressed.contains(code)).map(|(key, _)| *key).collect()
            }
            Some(fake) => fake.frames.get(fake.shown).map(|frame| frame.keys.clone()).unwrap_or_default(),
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

    /// How many pictures a fake window has been given to draw.
    pub fn pictures_drawn(&self) -> u32 {
        self.fake.as_ref().map_or(0, |fake| fake.pictures)
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
