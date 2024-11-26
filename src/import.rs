use image::{DynamicImage, GenericImageView, Pixel, Rgba};
use std::collections::HashMap;

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
