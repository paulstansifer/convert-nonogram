use image::{DynamicImage, GenericImageView, Pixel, Rgba};
use std::collections::HashMap;

use puzzle::Clue;

use crate::puzzle::{self, Color, Puzzle};

pub fn image_to_puzzle(image: &DynamicImage) -> Puzzle {
    let (width, height) = image.dimensions();

    let mut next_char = 'a';
    let mut palette = HashMap::<image::Rgba<u8>, Color>::new();

    // pbnsolve output looks weird if the default color isn't called "white".
    palette.insert(
        image::Rgba::<u8>([255, 255, 255, 255]),
        Color {
            ch: '.',
            name: "white".to_owned(),
            hex: "FFFFFF".to_owned(),
        },
    );

    let mut rows: Vec<Vec<Clue>> = Vec::new();
    let mut cols: Vec<Vec<Clue>> = Vec::new();

    let mut white_squares_found: u32 = 0;

    // Gather the palette
    for y in 0..height {
        for x in 0..width {
            let pixel: Rgba<u8> = image.get_pixel(x, y);
            let color = palette.entry(pixel).or_insert_with(|| {
                let this_char = next_char;
                let (r, g, b, _) = pixel.channels4();
                let hex = format!("{:02X}{:02X}{:02X}", r, g, b);

                next_char = (next_char as u8 + 1) as char;
                return Color {
                    ch: this_char,
                    name: format!("{}{}", this_char, hex),
                    hex: hex,
                };
            });

            if color.hex == "FFFFFF" {
                white_squares_found += 1;
            }
        }
    }

    if white_squares_found < (width + height) {
        eprintln!(
            "convert-nonogram: warning: {} is a very small number of white squares",
            white_squares_found
        );
    }

    if (width * height - white_squares_found) < (width + height) {
        eprintln!(
            "convert-nonogram: warning: {} is a very small number of non-white squares",
            width * height - white_squares_found
        );
    }

    if palette.len() > 30 {
        panic!(
            "{} colors detected. Nonograms with more than 30 colors are not supported.",
            palette.len()
        );
    } else if palette.len() > 10 {
        eprintln!(
            "convert-nonogram: {} colors detected. That's a very large number",
            palette.len()
        )
    }

    // Find similar colors
    for (rgba, color) in &palette {
        for (rgba2, color2) in &palette {
            if color == color2 {
                continue;
            }
            let (r, g, b, _) = rgba.channels4();
            let (r2, g2, b2, _) = rgba2.channels4();
            if (r2 as i16 - r as i16).abs()
                + (g2 as i16 - g as i16).abs()
                + (b2 as i16 - b as i16).abs()
                < 30
            {
                eprintln!(
                    "convert-nonogram: warning: very similar colors found: {} and {}",
                    color.hex, color2.hex
                );
            }
        }
    }

    // Generate row clues
    for y in 0..height {
        let mut clues = Vec::<Clue>::new();

        let mut cur_color: Option<&Color> = None;
        let mut run = 1;
        for x in 0..width + 1 {
            let color = if x < width {
                Some(&palette[&image.get_pixel(x, y)])
            } else {
                None
            };
            if cur_color == color {
                run += 1;
                continue;
            }
            match cur_color {
                None => {}
                Some(color) if color.hex == "FFFFFF" => {}
                Some(color) => clues.push(Clue {
                    color: color.clone(),
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
                Some(&palette[&image.get_pixel(x, y)])
            } else {
                None
            };
            if cur_color == color {
                run += 1;
                continue;
            }
            match cur_color {
                None => {}
                Some(color) if color.hex == "FFFFFF" => {}
                Some(color) => clues.push(Clue {
                    color: color.clone(),
                    count: run,
                }),
            }
            cur_color = color;
            run = 1;
        }
        cols.push(clues);
    }

    return Puzzle {
        palette,
        rows,
        cols,
    };
}
