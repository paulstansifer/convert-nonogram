extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod import;
mod line_solve;
mod puzzle;
mod gui; // Added gui module
use std::{io::Read, path::PathBuf};

use clap::Parser;
use crate::gui::EditorApp; // Added EditorApp import
use eframe; // Added eframe import
use import::webpbn_to_puzzle;
use puzzle::{Clue, Nono, Triano};

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
    #[arg(short, long, value_enum, default_value_t)]
    input_format: NonogramFormat,

    /// Format to emit as output
    #[arg(short, long, value_enum, default_value_t)]
    output_format: NonogramFormat,

    /// Explain the solve process line-by-line.
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    trace_solve: bool,

    /// Clue style (currently only meaningful for CharGrid input)
    #[arg(long, value_enum, default_value_t)]
    clue_style: ClueStyle,

    /// Launch the GUI editor
    #[clap(long)]
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

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    // It's important to use anyhow::Result for main when eframe is involved,
    // or handle its Result specifically. For now, let's change main's signature
    // and adjust the return type of this block.
    // The original main returns std::io::Result<()>. We'll need to adapt.
    // For simplicity, this diff won't change main's signature yet,
    // but will use .map_err for eframe::run_native.

    let (puzzle_result, solution_for_export) = match args.input_format {
        NonogramFormat::Html => {
            // Using anyhow::Result to make error handling more uniform
             Err(anyhow::anyhow!("HTML input is not supported."))
        }
        NonogramFormat::Image => {
            image::open(&args.input_path)
                .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))
                .map(|img| {
                    let solution = import::image_to_solution(&img);
                    (
                        Nono::to_dyn(import::solution_to_puzzle(&solution)),
                        Some(solution),
                    )
                })
        }
        NonogramFormat::Webpbn => {
            let webpbn_string = read_path(&args.input_path);
            // webpbn_to_puzzle itself might panic or return Result, ensure it's handled.
            // Assuming webpbn_to_puzzle returns Puzzle<Nono> directly (panics on error)
            let puzzle_data: puzzle::Puzzle<puzzle::Nono> = webpbn_to_puzzle(&webpbn_string);
            Ok((Nono::to_dyn(puzzle_data), None))
        }
        NonogramFormat::CharGrid => {
            let grid_string = read_path(&args.input_path);
            let solution = import::char_grid_to_solution(&grid_string);
            let puzzle_data = match args.clue_style {
                ClueStyle::Nono => Nono::to_dyn(import::solution_to_puzzle(&solution)),
                ClueStyle::Triano => Triano::to_dyn(import::solution_to_triano_puzzle(&solution)),
            };
            Ok((puzzle_data, Some(solution)))
        }
        _ => Err(anyhow::anyhow!("Unsupported input format combination.")),
    };

    // Handle puzzle loading result
    let (puzzle, solution) = match puzzle_result {
        Ok((p, s)) => (p, s),
        Err(e) => {
            eprintln!("Error loading puzzle: {}", e);
            std::process::exit(1);
        }
    };

    if args.gui {
        let app = EditorApp::new(puzzle); // puzzle is DynPuzzle
        let native_options = eframe::NativeOptions::default();
        // eframe::run_native returns Result<(), eframe::Error>, map it to std::io::Error for main
        eframe::run_native("Nonogram Editor", native_options, Box::new(|_cc| Box::new(app)))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Eframe error: {}", e)))?;
    } else {
        // Existing CLI logic
        match args.output_path {
            Some(path) => {
                if args.output_format == NonogramFormat::Image {
                    // Ensure solution is available for image export
                    if let Some(sol) = solution {
                        export::emit_image(&sol, path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Emit image error: {}", e)))?;
                    } else {
                        eprintln!("Error: Image output requires a solvable or direct image input.");
                        std::process::exit(1);
                    }
                } else {
                    let output_data = match args.output_format {
                        NonogramFormat::Olsak => export::as_olsak(&puzzle.clone().assume_nono()), // Clone if assume_nono consumes
                        NonogramFormat::Webpbn => export::as_webpbn(&puzzle.clone().assume_nono()),
                        NonogramFormat::Html => export::as_html(&puzzle.clone().assume_nono()),
                        NonogramFormat::Image => panic!("Handled above, this branch should not be reached."),
                        NonogramFormat::CharGrid => {
                            if let Some(sol) = solution {
                                export::as_char_grid(&sol)
                            } else {
                                eprintln!("Error: CharGrid output requires a solvable or direct image/char_grid input.");
                                std::process::exit(1);
                            }
                        },
                    };
                    if path == PathBuf::from("-") {
                        print!("{}", output_data);
                    } else {
                        std::fs::write(path, output_data)?;
                    }
                }
            }
            None => { // No output_path, solve the puzzle
                match puzzle.solve(args.trace_solve) {
                    Ok(_) => {} // Report is empty, success means it didn't error
                    Err(e) => {
                        eprintln!("Error solving puzzle: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    Ok(())
}
