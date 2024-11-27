use image::{DynamicImage, GenericImageView, Pixel, Rgba};
use std::{
    char::from_digit,
    collections::{BTreeMap, BTreeSet, HashMap},
};

use puzzle::Clue;

use crate::puzzle::{self, Color, ColorInfo, Puzzle, Solution, BACKGROUND};

pub fn image_to_solution(image: &DynamicImage) -> Solution {
    let (width, height) = image.dimensions();

    let mut palette = HashMap::<image::Rgba<u8>, ColorInfo>::new();
    let mut grid: Vec<Vec<Color>> = vec![vec![BACKGROUND; height as usize]; width as usize];

    // pbnsolve output looks weird if the default color isn't called "white".
    palette.insert(
        image::Rgba::<u8>([255, 255, 255, 255]),
        ColorInfo {
            ch: '.',
            name: "white".to_owned(),
            rgb: (255, 255, 255),
            color: puzzle::BACKGROUND,
        },
    );

    let mut next_char = 'a';
    let mut next_color_idx: u8 = 1; // BACKGROUND is 0

    // Gather the palette+
    for y in 0..height {
        for x in 0..width {
            let pixel: Rgba<u8> = image.get_pixel(x, y);
            let color = palette.entry(pixel).or_insert_with(|| {
                let this_char = next_char;
                let (r, g, b, _) = pixel.channels4();
                let this_color = Color(next_color_idx);

                next_char = (next_char as u8 + 1) as char;
                next_color_idx += 1;

                ColorInfo {
                    ch: this_char,
                    name: format!("{}{}", this_char, format!("{:02X}{:02X}{:02X}", r, g, b)),
                    rgb: (r, g, b),
                    color: this_color,
                }
            });

            grid[x as usize][y as usize] = color.color;
        }
    }

    Solution {
        palette: palette
            .into_values()
            .map(|color_info| (color_info.color, color_info))
            .collect(),
        grid,
    }
}

pub fn char_grid_to_solution(char_grid: &str) -> Solution {
    let mut palette = HashMap::<char, ColorInfo>::new();

    // We want deterministic behavior
    let mut unused_chars = BTreeSet::<char>::new();
    for ch in char_grid.chars() {
        if ch == '\n' {
            continue;
        }
        unused_chars.insert(ch);
    }

    let mut bg_ch: Option<char> = None;

    for possible_bg in [' ', '.', '_', 'w', 'W', '·', '☐', '0'] {
        if unused_chars.contains(&possible_bg) {
            bg_ch = Some(possible_bg);
        }
    }

    let bg_ch = match bg_ch {
        Some(x) => x,
        None => {
            eprintln!("convert-nonogram: Warning: unable to guess which character is supposed to be the background; using the upper-left corner");
            char_grid.trim_start().chars().next().unwrap()
        }
    };

    palette.insert(
        bg_ch,
        ColorInfo {
            ch: bg_ch,
            name: "white".to_string(),
            rgb: (255, 255, 255),
            color: BACKGROUND,
        },
    );
    unused_chars.remove(&bg_ch);

    let mut next_color: u8 = 1;

    for possible_black in ['#', 'B', 'b', '.', '■', '█', '1'] {
        if unused_chars.contains(&possible_black) {
            palette.insert(
                possible_black,
                ColorInfo {
                    ch: possible_black,
                    name: "black".to_string(),
                    rgb: (0, 0, 0),
                    color: Color(next_color),
                },
            );
            next_color += 1;
            unused_chars.remove(&possible_black);
            break;
        }
    }

    let mut unused_colors = BTreeMap::<char, (u8, u8, u8)>::new();
    unused_colors.insert('r', (255, 0, 0));
    unused_colors.insert('g', (0, 255, 0));
    unused_colors.insert('b', (0, 0, 255));

    unused_colors.insert('y', (255, 255, 0));
    unused_colors.insert('c', (0, 255, 255));
    unused_colors.insert('m', (255, 0, 255));

    for i in (1 as u8)..(9 as u8) {
        unused_colors.insert(from_digit(i.into(), 10).unwrap(), (22 * i, 22 * i, 22 * i));
    }

    for ch in unused_chars {
        let rgb = unused_colors
            .remove(&ch)
            .unwrap_or_else(|| unused_colors.pop_first().unwrap().1);

        palette.insert(
            ch,
            ColorInfo {
                ch: ch,
                name: ch.to_string(),
                rgb: rgb,
                color: Color(next_color),
            },
        );
        next_color += 1;
    }

    let mut grid: Vec<Vec<Color>> = vec![];

    // TODO: check that rows are the same length!
    for (y, row) in char_grid
        .split("\n")
        .filter(|line| !line.is_empty())
        .enumerate()
    {
        for (x, ch) in row.chars().enumerate() {
            // There's probably a better way than this...
            grid.resize(std::cmp::max(grid.len(), x + 1), vec![]);
            let new_height = std::cmp::max(grid[x].len(), y + 1);
            grid[x].resize(new_height, BACKGROUND);

            grid[x][y] = palette[&ch].color;
        }
    }

    Solution {
        palette: palette
            .into_values()
            .map(|color_info| (color_info.color, color_info))
            .collect(),
        grid,
    }
}

pub fn solution_to_puzzle(solution: Solution) -> Puzzle {
    let width = solution.grid.len();
    let height = solution.grid.first().unwrap().len();

    let mut rows: Vec<Vec<Clue>> = Vec::new();
    let mut cols: Vec<Vec<Clue>> = Vec::new();

    let bg_squares_found: usize = solution
        .grid
        .iter()
        .map(|col| {
            col.iter()
                .map(|c| if *c == BACKGROUND { 1 } else { 0 })
                .sum::<usize>()
        })
        .sum();

    if bg_squares_found < (width + height) {
        eprintln!(
            "convert-nonogram: warning: {} is a very small number of background squares",
            bg_squares_found
        );
    }

    if (width * height - bg_squares_found) < (width + height) {
        eprintln!(
            "convert-nonogram: warning: {} is a very small number of foreground squares",
            width * height - bg_squares_found
        );
    }

    let num_colors = solution.palette.len();
    if num_colors > 30 {
        panic!(
            "{} colors detected. Nonograms with more than 30 colors are not supported.",
            num_colors
        );
    } else if num_colors > 10 {
        eprintln!(
            "convert-nonogram: {} colors detected. That's probably too many.",
            num_colors
        )
    }

    // Find similar colors
    for (color_key, color) in &solution.palette {
        for (color_key2, color2) in &solution.palette {
            if color_key == color_key2 {
                continue;
            }
            let (r, g, b) = color.rgb;
            let (r2, g2, b2) = color2.rgb;
            if (r2 as i16 - r as i16).abs()
                + (g2 as i16 - g as i16).abs()
                + (b2 as i16 - b as i16).abs()
                < 30
            {
                eprintln!(
                    "convert-nonogram: warning: very similar colors found: {:?} and {:?}",
                    color.rgb, color2.rgb
                );
            }
        }
    }

    // Generate row clues
    for y in 0..height {
        let mut clues = Vec::<Clue>::new();

        let mut cur_color: Option<Color> = None;
        let mut run = 1;
        for x in 0..width + 1 {
            let color = if x < width {
                Some(solution.grid[x][y])
            } else {
                None
            };
            if cur_color == color {
                run += 1;
                continue;
            }
            match cur_color {
                None => {}
                Some(color) if color == puzzle::BACKGROUND => {}
                Some(color) => clues.push(Clue {
                    color: color,
                    count: run,
                }),
            }
            cur_color = color;
            run = 1;
        }
        rows.push(clues);
    }

    // Generate column clues
    for x in 0..width {
        let mut clues = Vec::<Clue>::new();

        let mut cur_color = None;
        let mut run = 1;
        for y in 0..height + 1 {
            let color = if y < height {
                Some(solution.grid[x][y])
            } else {
                None
            };
            if cur_color == color {
                run += 1;
                continue;
            }
            match cur_color {
                None => {}
                Some(color) if color == BACKGROUND => {}
                Some(color) => clues.push(Clue {
                    color: color,
                    count: run,
                }),
            }
            cur_color = color;
            run = 1;
        }
        cols.push(clues);
    }

    Puzzle {
        palette: solution.palette,
        rows,
        cols,
    }
}
