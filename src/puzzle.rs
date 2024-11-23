use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Clue {
    pub color: Color,
    pub count: u16,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct Color(pub u8);

pub static BACKGROUND: Color = Color(0);

#[derive(PartialEq, Eq, Clone)]
pub struct ColorInfo {
    pub ch: char,
    pub name: String,
    pub hex: String,
    pub color: Color,
}

pub struct Puzzle {
    pub palette: HashMap<Color, ColorInfo>, // should include the background!
    pub rows: Vec<Vec<Clue>>,
    pub cols: Vec<Vec<Clue>>,
}