extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod gui;
mod import;
mod line_solve;
mod puzzle;
use std::{io::Read, path::PathBuf};

use clap::Parser;
use import::{quality_check, webpbn_to_puzzle};
use puzzle::{Clue, Nono, Solution, Triano};

#[derive(Clone, Copy, Debug, clap::ValueEnum, Default, PartialEq, Eq)]
enum NonogramFormat {
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

fn read_path(path: &PathBuf) -> String {
    let mut res = String::new();
    if path == &PathBuf::from("-") {
        std::io::stdin()
            .read_to_string(&mut res)
            .expect("bad read_to_string!");
    } else {
        res = String::from_utf8(std::fs::read(path).unwrap()).expect("not valid UTF-8!");
    };
    res
}

fn infer_format(path: &PathBuf, format_arg: Option<NonogramFormat>) -> NonogramFormat {
    if let Some(format) = format_arg {
        return format;
    }

    match path.extension().and_then(|s| s.to_str()) {
        Some("png") | Some("bmp") | Some("gif") => NonogramFormat::Image,
        Some("webpbn") => NonogramFormat::Webpbn,
        Some("g") => NonogramFormat::Olsak,
        Some("html") => NonogramFormat::Html,
        Some("txt") => NonogramFormat::CharGrid,
        _ => NonogramFormat::CharGrid,
    }
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let input_format = infer_format(&args.input_path, args.input_format);

    let (puzzle, solution) = match input_format {
        NonogramFormat::Html => {
            panic!("HTML input is not supported.")
        }
        NonogramFormat::Image => {
            let img = image::open(args.input_path).unwrap();

            let solution = import::image_to_solution(&img);

            (
                Nono::to_dyn(import::solution_to_puzzle(&solution)),
                Some(solution),
            )
        }
        NonogramFormat::Webpbn => {
            let webpbn_string = read_path(&args.input_path);

            let puzzle: puzzle::Puzzle<puzzle::Nono> = webpbn_to_puzzle(&webpbn_string);

            (Nono::to_dyn(puzzle), None)
        }
        NonogramFormat::CharGrid => {
            let grid_string = read_path(&args.input_path);

            let mut solution = import::char_grid_to_solution(&grid_string);

            let puzzle = match args.clue_style {
                ClueStyle::Nono => Nono::to_dyn(import::solution_to_puzzle(&solution)),
                ClueStyle::Triano => {
                    let puzzle = import::solution_to_triano_puzzle(&solution);

                    // HACK: We adjusted the palette
                    solution.palette = puzzle.palette.clone();
                    Triano::to_dyn(puzzle)
                }
            };

            (puzzle, Some(solution))
        }
        _ => todo!(),
    };

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
