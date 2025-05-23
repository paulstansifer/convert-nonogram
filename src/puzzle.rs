use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Clue {
    pub color: Color,
    pub count: u16,
    pub pre_cap: Option<(Color, u16)>,
    pub post_cap: Option<(Color, u16)>,
}

impl Clue {
    pub fn total_length(&self) -> u16 {
        let mut length = self.count;
        if let Some((_, pre_len)) = self.pre_cap {
            length += pre_len;
        }
        if let Some((_, post_len)) = self.post_cap {
            length += post_len;
        }
        length
    }

    pub fn get_color_at_offset(&self, offset: u16) -> Color {
        let pre_cap_len = self.pre_cap.map_or(0, |(_, len)| len);
        let main_body_len = self.count;

        if self.pre_cap.is_some() && offset < pre_cap_len {
            return self.pre_cap.unwrap().0;
        } else if offset < pre_cap_len + main_body_len {
            return self.color;
        } else if self.post_cap.is_some() && offset < pre_cap_len + main_body_len + self.post_cap.unwrap().1 {
            return self.post_cap.unwrap().0;
        } else {
            panic!(
                "Offset {} out of bounds for clue with total_length {}",
                offset,
                self.total_length()
            );
        }
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
pub struct Puzzle {
    pub palette: HashMap<Color, ColorInfo>, // should include the background!
    pub rows: Vec<Vec<Clue>>,
    pub cols: Vec<Vec<Clue>>,
}
