use std::collections::HashMap;
use std::fmt::Debug;
pub trait Clue: Clone + Copy + Debug {
    fn new_solid(color: Color, count: u16) -> Self;

    fn must_be_separated_from(&self, next: &Self) -> bool;

    fn len(&self) -> usize;

    fn color_at(&self, idx: usize) -> Color;

    // Summary string (for display while solving)
    fn to_string(&self, puzzle: &Puzzle<Self>) -> String;

    // TODO: these are a hack!
    fn html_color(&self, puzzle: &Puzzle<Self>) -> String;

    fn html_text(&self, puzzle: &Puzzle<Self>) -> String;
}

impl Debug for Nono {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.color.0, self.count)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Nono {
    pub color: Color,
    pub count: u16,
}

impl Clue for Nono {
    fn new_solid(color: Color, count: u16) -> Self {
        Nono { color, count }
    }

    fn must_be_separated_from(&self, next: &Self) -> bool {
        self.color == next.color
    }

    fn len(&self) -> usize {
        self.count as usize
    }
    fn color_at(&self, _: usize) -> Color {
        self.color
    }

    fn to_string(&self, puzzle: &Puzzle<Self>) -> String {
        format!("{}{}", puzzle.palette[&self.color].ch, self.count)
    }

    fn html_color(&self, puzzle: &Puzzle<Self>) -> String {
        let (r, g, b) = puzzle.palette[&self.color].rgb;
        format!("color:rgb({},{},{})", r, g, b)
    }

    fn html_text(&self, _: &Puzzle<Self>) -> String {
        format!("{}", self.count)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Triano {
    pub front_cap: Option<Color>,
    pub body_len: u16,
    pub body_color: Color,
    pub back_cap: Option<Color>,
}

impl Clue for Triano {
    fn new_solid(color: Color, count: u16) -> Self {
        Triano {
            front_cap: None,
            body_len: count,
            body_color: color,
            back_cap: None,
        }
    }
    fn len(&self) -> usize {
        self.body_len as usize
            + self.front_cap.is_some() as usize
            + self.back_cap.is_some() as usize
    }
    fn color_at(&self, idx: usize) -> Color {
        if idx == 0 {
            self.front_cap.unwrap_or(self.body_color)
        } else if idx == self.len() - 1 {
            self.back_cap.unwrap_or(self.body_color)
        } else {
            self.body_color
        }
    }
    fn must_be_separated_from(&self, next: &Self) -> bool {
        // TODO: check the semantics with the book!
        self.body_color == next.body_color && self.back_cap.is_none() && next.front_cap.is_none()
    }

    fn to_string(&self, puzzle: &Puzzle<Self>) -> String {
        let mut res = String::new();
        if let Some(front_cap) = self.front_cap {
            res.push_str(&puzzle.palette[&front_cap].ch.to_string());
        }
        res.push_str(&puzzle.palette[&self.body_color].ch.to_string());
        res.push_str(&self.body_len.to_string());
        if let Some(back_cap) = self.back_cap {
            res.push_str(&puzzle.palette[&back_cap].ch.to_string());
        }
        res
    }

    fn html_color(&self, _: &Puzzle<Self>) -> String {
        unimplemented!()
    }

    fn html_text(&self, _: &Puzzle<Self>) -> String {
        unimplemented!()
    }
}

impl Debug for Triano {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(front_cap) = self.front_cap {
            write!(f, "{}", front_cap.0)?;
        }
        write!(f, "{}{}", self.body_color.0, self.body_len)?;
        if let Some(back_cap) = self.back_cap {
            write!(f, "{}", back_cap.0)?;
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct Color(pub u8);

pub static BACKGROUND: Color = Color(0);

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ColorInfo {
    pub ch: char,
    pub name: String,
    pub rgb: (u8, u8, u8),
    pub color: Color,
}

#[derive(Clone)]
pub struct Solution {
    pub palette: HashMap<Color, ColorInfo>, // should include the background!
    pub grid: Vec<Vec<Color>>,
}

#[derive(Clone)]
pub struct Puzzle<C: Clue> {
    pub palette: HashMap<Color, ColorInfo>, // should include the background!
    pub rows: Vec<Vec<C>>,
    pub cols: Vec<Vec<C>>,
}
