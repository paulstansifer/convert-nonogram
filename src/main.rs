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
use puzzle::NonogramFormat;

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

    /// Opens the GUI editor
    #[arg(long, default_value_t)]
    gui: bool,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let (puzzle, solution) = import::load(&args.input_path, args.input_format);
    if let Some(ref solution) = solution {
        quality_check(solution);
    }

    if args.gui {
        gui::edit_image(solution.unwrap());
        return Ok(());
    }

    match args.output_path {
        Some(path) => {
            export::save(Some(puzzle), solution.as_ref(), &path, args.output_format).unwrap();
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

#[test]
// This is a consistency test, used to notice when measured difficulties change.
fn solve_examples() {
    use crate::{grid_solve::Report, import};
    use itertools::Itertools;
    use std::path::PathBuf;

    let examples_dir = PathBuf::from("examples/png");
    let mut report = String::new();
    for entry in std::fs::read_dir(examples_dir)
        .unwrap()
        .into_iter()
        .sorted_by_key(|entry| entry.as_ref().unwrap().path().to_str().unwrap().to_string())
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let (puzzle, _solution) = import::load(&path, None);
            match puzzle.solve(false) {
                Ok(Report {
                    skims,
                    scrubs,
                    cells_left,
                    solution: _solution,
                    solved_mask: _solved_mask,
                }) => {
                    let filename = path.file_name().unwrap().to_str().unwrap();
                    report.push_str(&format!(
                        "{filename}: {skims} skims, {scrubs} scrubs, {cells_left} cells left\n"
                    ));
                }
                Err(e) => {
                    panic!("{path:?}: internal error: {:?}", e);
                }
            }
        }
    }
    println!("{}", report);

    assert!(report.contains("2:02.png: 167 skims, 16 scrubs, 0 cells left"));
    assert!(report.contains("apron.png: 77 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("bill_jeb_and_bob.png: 238 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("boring_blob.png: 32 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("boring_blob_large.png: 103 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("boring_hollow_blob.png: 34 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("carry_on_bag.png: 81 skims, 28 scrubs, 0 cells left"));
    assert!(
        report.contains("compact_fluorescent_lightbulb.png: 286 skims, 29 scrubs, 0 cells left")
    );
    assert!(report.contains("ear.png: 231 skims, 22 scrubs, 0 cells left"));
    assert!(report.contains("fire_submarine.png: 161 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("hair_dryer.png: 148 skims, 21 scrubs, 0 cells left"));
    assert!(report.contains("headphones.png: 430 skims, 1 scrubs, 0 cells left"));
    assert!(report.contains("keys.png: 62 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("ladle.png: 20 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("myst_falling_man.png: 64 skims, 14 scrubs, 0 cells left"));
    assert!(report.contains("pill_bottles.png: 243 skims, 14 scrubs, 0 cells left"));
    assert!(report.contains("puzzle_piece.png: 73 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("ringed_planet.png: 158 skims, 22 scrubs, 0 cells left"));
    assert!(report.contains("shirt_and_tie.png: 323 skims, 27 scrubs, 0 cells left"));
    assert!(report.contains("shirt_and_tie_no_button.png: 199 skims, 45 scrubs, 246 cells left"));
    assert!(report.contains("skid_steer.png: 209 skims, 1 scrubs, 0 cells left"));
    assert!(report.contains("sunglasses.png: 186 skims, 23 scrubs, 0 cells left"));
    assert!(report.contains("stroller.png: 125 skims, 76 scrubs, 406 cells left"));
    assert!(report.contains("tandem_stationary_bike.png: 365 skims, 50 scrubs, 0 cells left"));
    assert!(report.contains("tea.png: 100 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("tedious_dust.png: 91 skims, 22 scrubs, 0 cells left"));
    assert!(report.contains("tedious_dust_large.png: 521 skims, 89 scrubs, 0 cells left"));
    assert!(report.contains("telephone_recevier.png: 34 skims, 0 scrubs, 0 cells left"));
    assert!(report.contains("tissue_box.png: 65 skims, 49 scrubs, 148 cells left"));
    assert!(report.contains("tornado.png: 96 skims, 15 scrubs, 0 cells left"));
    assert!(report.contains("usb_type_a.png: 319 skims, 50 scrubs, 0 cells left"));
    assert!(report.contains("usb_type_a_no_emblem.png: 326 skims, 79 scrubs, 0 cells left"));

    assert_eq!(report.lines().collect::<Vec<_>>().len(), 32);
}
