extern crate clap;
extern crate image;

use image::{DynamicImage, GenericImageView, Pixel, Rgba};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone)]
struct Clue {
    color: Color,
    count: u8,
}

#[derive(PartialEq, Eq, Clone)]
struct Color {
    ch: char,
    name: String,
    hex: String,
}

// TODO: this is an awkward representation of a palette.
fn image_to_clues(
    image: &DynamicImage,
) -> (
    HashMap<image::Rgba<u8>, Color>,
    Vec<Vec<Clue>>,
    Vec<Vec<Clue>>,
) {
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

    let mut x_clues: Vec<Vec<Clue>> = Vec::new();
    let mut y_clues: Vec<Vec<Clue>> = Vec::new();

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
                    name: hex.clone(),
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

    if palette.len() > 20 {
        panic!(
            "{} colors detected. Nonograms with more than 20 colors are not supported. (Or fun.)",
            palette.len()
        );
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
        x_clues.push(clues);
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
        y_clues.push(clues);
    }

    return (palette, x_clues, y_clues);
}

fn emit_webpbn(
    palette: HashMap<image::Rgba<u8>, Color>,
    x_clues: Vec<Vec<Clue>>,
    y_clues: Vec<Vec<Clue>>,
) -> String {
    use indoc::indoc;

    let mut res = String::new();
    // If you add <!DOCTYPE pbn SYSTEM "https://webpbn.com/pbn-0.3.dtd">, `pbnsolve` emits a warning.
    res.push_str(indoc! {r#"
        <?xml version="1.0"?>
        <puzzleset>
        <puzzle type="grid" defaultcolor="white">
        <source>convert-nonogram</source>
        "#});
    for (_, color) in palette {
        res.push_str(&format!(
            r#"<color name="{}" char="{}">{}</color>"#,
            color.name, color.ch, color.hex
        ));
        res.push_str("\n");
    }

    res.push_str(r#"<clues type="columns">"#);
    for column in y_clues {
        res.push_str("<line>");
        for clue in column {
            res.push_str(&format!(
                r#"<count color="{}">{}</count>"#,
                clue.color.name, clue.count
            ));
        }
        res.push_str("</line>\n");
    }
    res.push_str(r#"</clues>"#);
    res.push_str("\n");

    res.push_str(r#"<clues type="rows">"#);
    for row in x_clues {
        res.push_str("<line>");
        for clue in row {
            res.push_str(&format!(
                r#"<count color="{}">{}</count>"#,
                clue.color.name, clue.count
            ));
        }
        res.push_str("</line>\n");
    }
    res.push_str(r#"</clues>"#);
    res.push_str("\n");

    res.push_str(r#"</puzzle></puzzleset>"#);
    res.push_str("\n");
    return res;
}

fn emit_olsak(
    palette: HashMap<image::Rgba<u8>, Color>,
    x_clues: Vec<Vec<Clue>>,
    y_clues: Vec<Vec<Clue>>,
) -> String {
    let mut res = String::new();
    res.push_str("#d\n");
    for (_, color) in palette {
        if color.hex == "FFFFFF" {
            res.push_str("   0:   #FFFFFF   white\n");
        } else {
            res.push_str(&format!(
                "   {}:{}  #{}   {}\n",
                color.ch, color.ch, color.hex, color.name
            ));
        }
    }
    res.push_str(": rows\n");
    for row in x_clues {
        for clue in row {
            res.push_str(&format!("{}{} ", clue.count, clue.color.ch));
        }
        res.push_str("\n");
    }
    res.push_str(": columns\n");
    for column in y_clues {
        for clue in column {
            res.push_str(&format!("{}{} ", clue.count, clue.color.ch));
        }
        res.push_str("\n");
    }
    return res;
}

fn main() -> std::io::Result<()> {
    let matches = clap::App::new("convert-nonogram")
        .version("0.1.1")
        .author("Paul Stansifer")
        .about("Converts images of nonogram solutions to puzzles")
        .arg(
            clap::Arg::with_name("INPUT")
                .help("image input file")
                .required(true),
        )
        .arg(
            clap::Arg::with_name("OUTPUT")
                .long("output")
                .short("o")
                .takes_value(true)
                .help("output file (outputs to stdout if omitted)"),
        )
        .arg(
            clap::Arg::with_name("olsak")
                .long("olsak")
                .help("emit nonogram in the 'olsak' format"),
        )
        .get_matches();
    let img = image::open(matches.value_of("INPUT").unwrap()).unwrap();

    let (palette, x_clues, y_clues) = image_to_clues(&img);

    let output = if matches.is_present("olsak") {
        emit_olsak(palette, x_clues, y_clues)
    } else {
        emit_webpbn(palette, x_clues, y_clues)
    };

    match matches.value_of("OUTPUT") {
        None => {
            print!("{}", output);
        }
        Some(filename) => {
            std::fs::write(filename, output)?;
        }
    }

    Ok(())
}
