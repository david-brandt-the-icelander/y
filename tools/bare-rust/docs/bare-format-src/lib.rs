use std::collections::HashMap;
use std::fmt::Debug;

pub struct Counter {
    counts: HashMap<String, u32>,
}

pub struct Point {
    pub x: f64,
    y: f64,
}

pub struct Meters(pub f64, f64);

pub enum Shape {
    Dot,
    Circle(f64),
    Rect { w: f64, h: f64 },
}

pub type Table = HashMap<String, u32>;

pub trait Describe {
    fn describe(&self) -> String;
}

impl Describe for Counter {
    fn describe(&self) -> String {
        format!("{} words", self.counts.len())
    }
}

impl Counter {
    pub fn get(&self, word: &str) -> Option<&u32> {
        self.counts.get(word)
    }

    pub fn bump(&mut self, word: String) {
        *self.counts.entry(word).or_insert(0) += 1;
    }
}

pub fn double(x: u32) -> u32 {
    x * 2
}

pub fn largest<T: PartialOrd + Copy>(items: &[T]) -> T {
    let mut best = items[0];
    for &x in items {
        if x > best {
            best = x;
        }
    }
    best
}

pub fn show_all<T>(items: &[T]) -> Vec<String>
where
    T: Debug,
{
    items.iter().map(|x| format!("{x:?}")).collect::<Vec<String>>()
}

pub fn mean(values: &[f64]) -> f64 {
    let total: f64 = values.iter().sum();
    let n: f64 = values.len() as f64;
    total / n
}
