extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod gui;
mod import;
mod line_solve;
mod puzzle;
use std::path::PathBuf;

use clap::Parser;
use import::quality_check;

#[derive(Clone, Copy, Debug, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum NonogramFormat {
    #[default]
    /// Any image supported by the `image` crate (when used as output, infers format from
    /// extension).
    Image,
    /// The widely-used format associated with http://webpbn.com.
    Webpbn,
    /// (Export-only.) The format used by the 'olsak' solver.
    Olsak,
    /// Informal text format: a grid of characters. Attempts some sensible matching of characters
    /// to colors, but results will vary. This is the only format that supports Triano puzzles.
    CharGrid,
    /// (Export-only.) An HTML representation of a puzzle.
    Html,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, Default, PartialEq, Eq)]
enum ClueStyle {
    #[default]
    Nono,
    Triano,
}

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input path; use "-" for stdin
    input_path: PathBuf,

    /// Output path for format conversion; use "-" for stdout.
    /// If omitted, solves the nonogram and reports on the difficulty.
    output_path: Option<PathBuf>,

    /// Format to expect the input to be in
    #[arg(short, long, value_enum)]
    input_format: Option<NonogramFormat>,

    /// Format to emit as output
    #[arg(short, long, value_enum)]
    output_format: Option<NonogramFormat>,

    /// Explain the solve process line-by-line.
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    trace_solve: bool,

    /// Clue style (currently only meaningful for CharGrid input)
    #[arg(long, value_enum, default_value_t)]
    clue_style: ClueStyle,

    /// Opens the GUI editor
    #[arg(long, default_value_t)]
    gui: bool,
}

pub fn infer_format(path: &PathBuf, format_arg: Option<NonogramFormat>) -> NonogramFormat {
    if let Some(format) = format_arg {
        return format;
    }

    match path.extension().and_then(|s| s.to_str()) {
        Some("png") | Some("bmp") | Some("gif") => NonogramFormat::Image,
        Some("xml") | Some("pbn") => NonogramFormat::Webpbn,
        Some("g") => NonogramFormat::Olsak,
        Some("html") => NonogramFormat::Html,
        Some("txt") => NonogramFormat::CharGrid,
        _ => NonogramFormat::CharGrid,
    }
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let (puzzle, solution) = import::load(&args.input_path, args.input_format, args.clue_style);
    if let Some(ref solution) = solution {
        quality_check(solution);
    }

    if args.gui {
        gui::edit_image(&mut solution.unwrap(), args.clue_style);
        return Ok(());
    }

    match args.output_path {
        Some(path) => {
            let output_format = infer_format(&path, args.output_format);

            if output_format == NonogramFormat::Image {
                export::emit_image(&solution.unwrap(), path).unwrap();
            } else {
                let output_data = match output_format {
                    NonogramFormat::Olsak => export::as_olsak(&puzzle.assume_nono()),
                    NonogramFormat::Webpbn => export::as_webpbn(&puzzle.assume_nono()),
                    NonogramFormat::Html => match puzzle {
                        puzzle::DynPuzzle::Nono(p) => export::as_html(&p),
                        puzzle::DynPuzzle::Triano(p) => export::as_html(&p),
                    },
                    NonogramFormat::Image => panic!(),
                    NonogramFormat::CharGrid => export::as_char_grid(&solution.unwrap()),
                };
                if path == PathBuf::from("-") {
                    print!("{}", output_data);
                } else {
                    std::fs::write(path, output_data)?;
                }
            }
        }

        None => match puzzle.solve(args.trace_solve) {
            Ok(grid_solve::Report {
                skims,
                scrubs,
                cells_left,
                solution: _solution,
                solved_mask: _solved_mask,
            }) => {
                if cells_left == 0 {
                    eprintln!("Solved after {} skims and {} scrubs.", skims, scrubs);
                } else {
                    eprintln!(
                        "Unable to solve. Performed {} skims, {} scrubs; {} cells left.",
                        skims, scrubs, cells_left
                    );
                }
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        },
    }

    Ok(())
}
