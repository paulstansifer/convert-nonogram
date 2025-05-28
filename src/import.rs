use anyhow::bail;
use image::{DynamicImage, GenericImageView, Pixel, Rgba};
use std::{
    char::from_digit,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    iter::FromIterator,
};

use crate::puzzle::{self, Color, ColorInfo, Corner, Nono, Puzzle, Solution, Triano, BACKGROUND};

pub fn image_to_solution(image: &DynamicImage) -> Solution {
    let (width, height) = image.dimensions();

    let mut palette = HashMap::<image::Rgba<u8>, ColorInfo>::new();
    let mut grid: Vec<Vec<Color>> = vec![vec![BACKGROUND; height as usize]; width as usize];

    // pbnsolve output looks weird if the default color isn't called "white".
    palette.insert(
        image::Rgba::<u8>([255, 255, 255, 255]),
        ColorInfo {
            ch: ' ',
            name: "white".to_owned(),
            rgb: (255, 255, 255),
            color: puzzle::BACKGROUND,
            corner: None,
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

                next_color_idx += 1;

                if r == 0 && g == 0 && b == 0 {
                    return ColorInfo {
                        ch: '◼',
                        name: "black".to_string(),
                        rgb: (0, 0, 0),
                        color: this_color,
                        corner: None,
                    };
                }

                next_char = (next_char as u8 + 1) as char;

                ColorInfo {
                    ch: this_char,
                    name: format!("{}{}", this_char, format!("{:02X}{:02X}{:02X}", r, g, b)),
                    rgb: (r, g, b),
                    color: this_color,
                    corner: None,
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

    // Look for a character that seems to represent a white background.
    for possible_bg in [' ', '.', '_', 'w', 'W', '·', '☐', '0'] {
        if unused_chars.contains(&possible_bg) {
            bg_ch = Some(possible_bg);
        }
    }

    // But we need to *some* color as background to proceed!
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
            corner: None,
        },
    );
    unused_chars.remove(&bg_ch);

    let mut next_color: u8 = 1;

    // Look for a character that might be black (but it's not required to exist).
    for possible_black in ['#', 'B', 'b', '.', '■', '█', '1'] {
        if unused_chars.contains(&possible_black) {
            palette.insert(
                possible_black,
                ColorInfo {
                    ch: possible_black,
                    name: "black".to_string(),
                    rgb: (0, 0, 0),
                    color: Color(next_color),
                    corner: None,
                },
            );
            next_color += 1;
            unused_chars.remove(&possible_black);
            break;
        }
    }

    let lower_right_tri = HashSet::<char>::from_iter(['◢', '🮞', '◿']);
    let lower_left_tri = HashSet::<char>::from_iter(['◣', '🮟', '◺']);
    let upper_left_tri = HashSet::<char>::from_iter(['◤', '🮜', '◸']);
    let upper_right_tri = HashSet::<char>::from_iter(['◥', '🮝', '◹']);
    let mut any_tri = HashSet::<char>::new();
    any_tri.extend(lower_right_tri.iter());
    any_tri.extend(lower_left_tri.iter());
    any_tri.extend(upper_left_tri.iter());
    any_tri.extend(upper_right_tri.iter());

    // By default, use primary and secondary colors:
    let mut unused_colors = BTreeMap::<char, (u8, u8, u8)>::new();
    unused_colors.insert('r', (255, 0, 0));
    unused_colors.insert('g', (0, 255, 0));
    unused_colors.insert('b', (0, 0, 255));

    unused_colors.insert('y', (255, 255, 0));
    unused_colors.insert('c', (0, 255, 255));
    unused_colors.insert('m', (255, 0, 255));

    for ch in unused_chars {
        if unused_colors.is_empty() {
            // If desperate, use grays and dark colors:
            for i in 1_u8..5_u8 {
                unused_colors.insert(from_digit(i.into(), 10).unwrap(), (44 * i, 44 * i, 44 * i));
            }
            unused_colors.insert('R', (127, 0, 0));
            unused_colors.insert('G', (0, 127, 0));
            unused_colors.insert('B', (0, 0, 127));

            unused_colors.insert('Y', (127, 127, 0));
            unused_colors.insert('C', (0, 127, 127));
            unused_colors.insert('M', (127, 0, 127));
        }
        let rgb = unused_colors
            .remove(&ch)
            .unwrap_or_else(|| unused_colors.pop_first().unwrap().1);

        palette.insert(
            ch,
            ColorInfo {
                ch,
                name: ch.to_string(),
                rgb,
                color: Color(next_color),
                corner: if any_tri.contains(&ch) {
                    Some(Corner {
                        upper: upper_left_tri.contains(&ch) || upper_right_tri.contains(&ch),
                        left: lower_left_tri.contains(&ch) || upper_left_tri.contains(&ch),
                    })
                } else {
                    None
                },
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

pub fn get_children<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    tag: &str,
) -> anyhow::Result<Vec<roxmltree::Node<'a, 'input>>> {
    let mut res = vec![];

    for child in node.children() {
        if child.is_text() {
            if child.text().unwrap().trim() != "" {
                bail!("unexpected text: {}", child.text().unwrap());
            }
        }
        if child.is_element() {
            if child.tag_name().name() == tag {
                res.push(child);
            } else {
                bail!(
                    "unexpected element {}; was looking for {tag}",
                    child.tag_name().name()
                )
            }
        }
    }

    Ok(res)
}

pub fn get_single_child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    tag: &str,
) -> anyhow::Result<roxmltree::Node<'a, 'input>> {
    let mut res = get_children(node, tag)?;
    if res.len() == 0 {
        bail!("did not find the element {tag}");
    }
    if res.len() > 1 {
        bail!("expected only one element named {tag}");
    }
    Ok(res.pop().unwrap())
}

pub fn webpbn_to_puzzle(webpbn: &str) -> Puzzle<Nono> {
    let doc = roxmltree::Document::parse(webpbn).unwrap();
    let puzzleset = doc.root_element();
    let puzzle = get_single_child(puzzleset, "puzzle").unwrap();

    let default_color = puzzle
        .attribute("defaultcolor")
        .expect("Expected a 'defaultcolor");
    let mut next_color_index = 1;

    let mut named_colors = HashMap::<String, Color>::new();

    let mut res = Puzzle {
        palette: HashMap::<Color, ColorInfo>::new(),
        rows: vec![],
        cols: vec![],
    };

    for puzzle_part in puzzle.children() {
        if puzzle_part.tag_name().name() == "color" {
            let color_name = puzzle_part.attribute("name").unwrap();
            let color = if color_name == default_color {
                BACKGROUND
            } else {
                Color(next_color_index)
            };

            if color != BACKGROUND {
                next_color_index += 1
            }

            let hex_color = regex::Regex::new(
                r"([0-9A-Za-z][0-9A-Za-z])([0-9A-Za-z][0-9A-Za-z])([0-9A-Za-z][0-9A-Za-z])",
            )
            .unwrap();

            let color_text = puzzle_part.text().expect("Expected hex color in text");
            let (_, component_strs) = hex_color
                .captures(&color_text)
                .expect("Expected a string of 6 hex digits")
                .extract();

            let [r, g, b] = component_strs.map(|s| u8::from_str_radix(s, 16).unwrap());

            let color_info = ColorInfo {
                // TODO: error if there's more than one char!
                ch: puzzle_part
                    .attribute("char")
                    .unwrap()
                    .chars()
                    .next()
                    .unwrap(),
                name: color_name.to_string(),
                rgb: (r, g, b),
                color: color,
                corner: None, // webpbn isn't intended to represent Triano clues
            };

            res.palette.insert(color, color_info);
            named_colors.insert(color_name.to_string(), color);
        } else if puzzle_part.tag_name().name() == "clues" {
            let row = if puzzle_part.attribute("type") == Some("rows") {
                true
            } else if puzzle_part.attribute("type") == Some("columns") {
                false
            } else {
                panic!("Expected rows or columns.")
            };

            let mut clue_lanes = vec![];

            for lane in get_children(puzzle_part, "line").unwrap() {
                let mut clues = vec![];
                for block in get_children(lane, "count").unwrap() {
                    clues.push(Nono {
                        color: named_colors[block
                            .attribute("color")
                            .expect("Expected 'color' attribute")],
                        count: u16::from_str_radix(&block.text().unwrap(), 10)
                            .expect("Expected a number."),
                    });
                }
                clue_lanes.push(clues);
            }

            if row {
                res.rows = clue_lanes;
            } else {
                res.cols = clue_lanes;
            }
        }
    }

    res
}

fn quality_check(solution: &Solution) {
    let width = solution.grid.len();
    let height = solution.grid.first().unwrap().len();

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
}

pub fn solution_to_triano_puzzle(solution: &Solution) -> Puzzle<Triano> {
    let width = solution.grid.len();
    let height = solution.grid.first().unwrap().len();

    quality_check(solution);

    let mut rows: Vec<Vec<Triano>> = Vec::new();
    let mut cols: Vec<Vec<Triano>> = Vec::new();

    let blank_clue = Triano {
        front_cap: None,
        body_color: BACKGROUND,
        body_len: 0,
        back_cap: None,
    };

    let mut palette = solution.palette.clone();

    // Let's assume triano clues are black-and-white
    for (color, color_info) in &mut palette {
        if color == &BACKGROUND {
            continue;
        }
        color_info.rgb = (0, 0, 0);
    }

    // Generate row clues
    for y in 0..height {
        let mut clues = Vec::<Triano>::new();
        let mut cur_clue = blank_clue;

        for x in 0..width {
            let color = solution.grid[x][y];
            let color_info = &palette[&color];

            // For example `!left` means ◢ or ◥:
            if color_info.corner.is_some_and(|c| !c.left) {
                // Only a blank clue can accept a front cap:
                if cur_clue != blank_clue {
                    clues.push(cur_clue);
                    cur_clue = blank_clue
                }
                cur_clue.front_cap = Some(color);
            } else if color_info.corner.is_some_and(|c| c.left) {
                // The back cap is always none...
                cur_clue.back_cap = Some(color);
                // ...because we finish right after setting it
                clues.push(cur_clue);
                cur_clue = blank_clue;
            } else if color == BACKGROUND {
                if cur_clue != blank_clue {
                    clues.push(cur_clue);
                    cur_clue = blank_clue;
                }
            } else {
                // Since the back cap is always none, the only obstacle to continuing is if the
                // body color is wrong.
                if cur_clue.body_color != BACKGROUND && cur_clue.body_color != color {
                    clues.push(cur_clue);
                    cur_clue = blank_clue;
                }
                cur_clue.body_color = color;
                cur_clue.body_len += 1;
            }
        }
        if cur_clue != blank_clue {
            clues.push(cur_clue);
        }

        rows.push(clues);
    }

    // Generate column clues
    for x in 0..width {
        let mut clues = Vec::<Triano>::new();
        let mut cur_clue = blank_clue;

        for y in 0..height {
            let color = solution.grid[x][y];
            let color_info = &palette[&color];

            if color_info.corner.is_some_and(|c| !c.upper) {
                // Only a blank clue can accept a front cap:
                if cur_clue != blank_clue {
                    clues.push(cur_clue);
                    cur_clue = blank_clue
                }
                cur_clue.front_cap = Some(color);
            } else if color_info.corner.is_some_and(|c| c.upper) {
                // The back cap is always none...
                cur_clue.back_cap = Some(color);
                // ...because we finish right after setting it
                clues.push(cur_clue);
                cur_clue = blank_clue;
            } else if color == BACKGROUND {
                if cur_clue != blank_clue {
                    clues.push(cur_clue);
                    cur_clue = blank_clue;
                }
            } else {
                // Since the back cap is always none, the only obstacle to continuing is if the
                // body color is wrong.
                if cur_clue.body_color != BACKGROUND && cur_clue.body_color != color {
                    clues.push(cur_clue);
                    cur_clue = blank_clue;
                }
                cur_clue.body_color = color;
                cur_clue.body_len += 1;
            }
        }
        if cur_clue != blank_clue {
            clues.push(cur_clue);
        }

        cols.push(clues);
    }

    Puzzle {
        palette,
        rows,
        cols,
    }
}

pub fn solution_to_puzzle(solution: &Solution) -> Puzzle<Nono> {
    let width = solution.grid.len();
    let height = solution.grid.first().unwrap().len();

    quality_check(solution);

    let mut rows: Vec<Vec<Nono>> = Vec::new();
    let mut cols: Vec<Vec<Nono>> = Vec::new();

    // Generate row clues
    for y in 0..height {
        let mut clues = Vec::<Nono>::new();

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
                Some(color) => clues.push(Nono { color, count: run }),
            }
            cur_color = color;
            run = 1;
        }
        rows.push(clues);
    }

    // Generate column clues
    for x in 0..width {
        let mut clues = Vec::<Nono>::new();

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
                Some(color) => clues.push(Nono { color, count: run }),
            }
            cur_color = color;
            run = 1;
        }
        cols.push(clues);
    }

    Puzzle {
        palette: solution.palette.clone(),
        rows,
        cols,
    }
}
