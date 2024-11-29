use std::path::Path;

use image::{Rgb, RgbImage};

use crate::puzzle::{Puzzle, Solution};

pub fn as_webpbn(puzzle: &Puzzle) -> String {
    use indoc::indoc;

    let mut res = String::new();
    // If you add <!DOCTYPE pbn SYSTEM "https://webpbn.com/pbn-0.3.dtd">, `pbnsolve` emits a warning.
    res.push_str(indoc! {r#"
        <?xml version="1.0"?>
        <puzzleset>
        <puzzle type="grid" defaultcolor="white">
        <source>convert-nonogram</source>
        "#});
    for color in puzzle.palette.values() {
        let (r, g, b) = color.rgb;
        res.push_str(&format!(
            r#"<color name="{}" char="{}">{:02X}{:02X}{:02X}</color>"#,
            color.name, color.ch, r, g, b
        ));
        res.push('\n');
    }

    res.push_str(r#"<clues type="columns">"#);
    for column in &puzzle.cols {
        res.push_str("<line>");
        for clue in column {
            res.push_str(&format!(
                r#"<count color="{}">{}</count>"#,
                puzzle.palette[&clue.color].name, clue.count
            ));
        }
        res.push_str("</line>\n");
    }
    res.push_str(r#"</clues>"#);
    res.push('\n');

    res.push_str(r#"<clues type="rows">"#);
    for row in &puzzle.rows {
        res.push_str("<line>");
        for clue in row {
            res.push_str(&format!(
                r#"<count color="{}">{}</count>"#,
                puzzle.palette[&clue.color].name, clue.count
            ));
        }
        res.push_str("</line>\n");
    }
    res.push_str(r#"</clues>"#);
    res.push('\n');

    res.push_str(r#"</puzzle></puzzleset>"#);
    res.push('\n');

    res
}

pub fn as_olsak(puzzle: &Puzzle) -> String {
    let mut res = String::new();
    res.push_str("#d\n");

    // Nonny doesn't like it if white isn't the first color in the palette.
    res.push_str("   0:   #FFFFFF   white\n");
    for color in puzzle.palette.values() {
        if color.rgb != (255, 255, 255) {
            let (r, g, b) = color.rgb;
            res.push_str(&format!(
                "   {}:{}  #{:02X}{:02X}{:02X}   {}\n",
                color.ch, color.ch, r, g, b, color.name
            ));
        }
    }
    res.push_str(": rows\n");
    for row in &puzzle.rows {
        for clue in row {
            res.push_str(&format!(
                "{}{} ",
                clue.count, puzzle.palette[&clue.color].ch
            ));
        }
        res.push('\n');
    }
    res.push_str(": columns\n");
    for column in &puzzle.cols {
        for clue in column {
            res.push_str(&format!(
                "{}{} ",
                clue.count, puzzle.palette[&clue.color].ch
            ));
        }
        res.push('\n');
    }

    res
}

pub fn emit_image<P>(solution: &Solution, path: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let mut image = RgbImage::new(
        solution.grid.len() as u32,
        solution.grid.first().unwrap().len() as u32,
    );

    for (x, col) in solution.grid.iter().enumerate() {
        for (y, color) in col.iter().enumerate() {
            let color_info = &solution.palette[color];
            let (r, g, b) = color_info.rgb;
            image.put_pixel(x as u32, y as u32, Rgb::<u8>([r, g, b]));
        }
    }

    Ok(image.save(path)?)
}
