extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod import;
mod line_solve;
mod puzzle;
use std::{io::Read, path::PathBuf};

use clap::Parser;

#[derive(Clone, Copy, Debug, clap::ValueEnum, Default)]
enum NonogramFormat {
    #[default]
    /// Any image supported by the `image` crate (when used as output, defaults to `.png`)
    Image,
    /// A grid of characters. Some characters have default colors associated with them;
    /// others are chosen arbitrarily.
    CharGrid,
    /// The format used by the 'olsak' solver.
    Olsak,
    /// The widely-used format associated with http://webpbn.com.
    Webpbn,
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
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let puzzle = match args.input_format {
        NonogramFormat::Image => {
            let img = image::open(args.input_path).unwrap();

            import::solution_to_puzzle(import::image_to_solution(&img))
        }
        NonogramFormat::CharGrid => {
            let mut grid_string = String::new();
            if args.input_path == PathBuf::from("-") {
                std::io::stdin()
                    .read_to_string(&mut grid_string)
                    .expect("bad read_to_string!");
            } else {
                grid_string = String::from_utf8(std::fs::read(args.input_path).unwrap())
                    .expect("not valid UTF-8!");
            };

            import::solution_to_puzzle(import::char_grid_to_solution(&grid_string))
        }
        _ => todo!(),
    };

    match args.output_path {
        Some(path) => {
            let output_data = match args.output_format {
                NonogramFormat::Olsak => export::emit_olsak(&puzzle),
                NonogramFormat::Webpbn => export::emit_webpbn(&puzzle),
                // NonogramFormat::Image =>
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

        None => {
            grid_solve::solve(&puzzle).unwrap();
        }
    }

    Ok(())
}
