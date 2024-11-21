use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone)]
pub struct Clue {
    pub color: Color,
    pub count: u8,
}

#[derive(PartialEq, Eq, Clone)]
pub struct Color {
    pub ch: char,
    pub name: String,
    pub hex: String,
}

pub struct Puzzle {
    // TODO: this is kinda an awkward representation, driven by input images. Refactor!
    pub palette: HashMap<image::Rgba<u8>, Color>,
    pub rows: Vec<Vec<Clue>>,
    pub cols: Vec<Vec<Clue>>,
}
