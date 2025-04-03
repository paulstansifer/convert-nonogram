extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod import;
mod line_solve;
mod puzzle;
use std::{io::Read, path::PathBuf};

use clap::Parser;
use import::webpbn_to_puzzle;

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
    /// A grid of characters. Attempts some sensible matching of characters to colors, but results
    /// will vary.
    CharGrid,
    /// (Export-only.) An HTML representation of a puzzle.
    Html,
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
    #[arg(short, long, value_enum, default_value_t)]
    input_format: NonogramFormat,

    /// Format to emit as output
    #[arg(short, long, value_enum, default_value_t)]
    output_format: NonogramFormat,

    /// Explain the solve process line-by-line.
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    trace_solve: bool,
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

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let (puzzle, solution) = match args.input_format {
        NonogramFormat::Html => {
            panic!("HTML input is not supported.")
        }
        NonogramFormat::Image => {
            let img = image::open(args.input_path).unwrap();

            let solution = import::image_to_solution(&img);

            (import::solution_to_puzzle(&solution), Some(solution))
        }
        NonogramFormat::Webpbn => {
            let webpbn_string = read_path(&args.input_path);

            let puzzle = webpbn_to_puzzle(&webpbn_string);

            (puzzle, None)
        }
        NonogramFormat::CharGrid => {
            let grid_string = read_path(&args.input_path);

            let solution = import::char_grid_to_solution(&grid_string);

            (import::solution_to_puzzle(&solution), Some(solution))
        }
        _ => todo!(),
    };

    match args.output_path {
        Some(path) => {
            if args.output_format == NonogramFormat::Image {
                export::emit_image(&solution.unwrap(), path).unwrap();
            } else {
                let output_data = match args.output_format {
                    NonogramFormat::Olsak => export::as_olsak(&puzzle),
                    NonogramFormat::Webpbn => export::as_webpbn(&puzzle),
                    NonogramFormat::Html => export::as_html(&puzzle),
                    NonogramFormat::Image => panic!(),
                    _ => {
                        todo!()
                    }
                };
                if path == PathBuf::from("-") {
                    print!("{}", output_data);
                } else {
                    std::fs::write(path, output_data)?;
                }
            }
        }

        None => match grid_solve::solve(&puzzle, args.trace_solve) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        },
    }

    Ok(())
}
