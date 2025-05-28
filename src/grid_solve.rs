use colored::Colorize;
use ndarray::{ArrayView1, ArrayViewMut1};

use crate::{
    line_solve::{scrub_heuristic, scrub_line, skim_heuristic, skim_line, Cell},
    puzzle::{Clue, Puzzle},
};

type Grid = ndarray::Array2<Cell>;

pub struct Report {}

pub struct LaneState<'a, C: Clue> {
    clues: &'a [C], // just convenience, since `row` and `index` suffice to find it again
    row: bool,
    index: ndarray::Ix,
    scrubbed: bool,
    scrub_score: i32,
    processed_scrub_score: i32,
    skimmed: bool,
    skim_score: i32,
    processed_skim_score: i32,
}

impl<'a, C: Clue> LaneState<'a, C> {
    pub fn text_coord(&self) -> String {
        format!("{}{}", if self.row { "R" } else { "C" }, self.index + 1)
    }

    fn new(clues: &'a [C], row: bool, idx: usize, grid: &Grid) -> LaneState<'a, C> {
        let mut res = LaneState {
            clues,
            row,
            index: idx,
            scrubbed: false,
            scrub_score: 0,
            processed_scrub_score: 0,
            skimmed: false,
            skim_score: 0,
            processed_skim_score: 0,
        };
        res.rescore(grid, false);
        res
    }
    fn rescore(&mut self, grid: &Grid, was_processed: bool) {
        let lane = get_grid_lane(self, grid);
        if lane.iter().all(|cell| cell.is_known()) {
            self.scrub_score = std::i32::MIN;
            self.skim_score = std::i32::MIN;
            return;
        }
        if was_processed {
            self.processed_scrub_score = self.scrub_score;
            self.processed_skim_score = self.skim_score;
        }
        self.scrub_score = scrub_heuristic(self.clues, lane);
        self.skim_score = skim_heuristic(self.clues, lane);
    }

    fn effective_score(&self, to_scrub: bool) -> i32 {
        if to_scrub {
            self.scrub_score.saturating_sub(self.processed_scrub_score)
        } else {
            self.skim_score.saturating_sub(self.processed_skim_score)
        }
    }
}

impl<'a, C: Clue> std::cmp::PartialEq for LaneState<'a, C> {
    fn eq(&self, other: &Self) -> bool {
        self.scrubbed == other.scrubbed && self.scrub_score == other.scrub_score
    }
}

impl<'a, C: Clue> std::cmp::Eq for LaneState<'a, C> {}

fn get_mut_grid_lane<'a, C: Clue>(
    ls: &LaneState<'a, C>,
    grid: &'a mut Grid,
) -> ArrayViewMut1<'a, Cell> {
    if ls.row {
        grid.row_mut(ls.index)
    } else {
        grid.column_mut(ls.index)
    }
}

fn get_grid_lane<'a, C: Clue>(ls: &LaneState<'a, C>, grid: &'a Grid) -> ArrayView1<'a, Cell> {
    if ls.row {
        grid.row(ls.index)
    } else {
        grid.column(ls.index)
    }
}

fn find_best_lane<'a, 'b, C: Clue>(
    lanes: &'b mut [LaneState<'a, C>],
    to_scrub: bool,
) -> Option<&'b mut LaneState<'a, C>> {
    let mut best_score = std::i32::MIN;
    let mut res = None;

    for lane in lanes {
        if to_scrub && lane.scrubbed || (!to_scrub) && lane.skimmed {
            continue;
        }

        if lane.effective_score(to_scrub) > best_score {
            best_score = lane.effective_score(to_scrub);
            res = Some(lane);
        }
    }
    res
}

fn print_grid<C: Clue>(grid: &Grid, puzzle: &Puzzle<C>) {
    for row in grid.rows() {
        for cell in row {
            match cell.known_or() {
                None => {
                    print!("?");
                }
                Some(c) => {
                    print!("{}", puzzle.palette[&c].ch);
                }
            }
        }
        println!();
    }
}

fn display_step<'a, C: Clue>(
    clue_lane: &'a LaneState<'a, C>,
    orig_lane: Vec<Cell>,
    scrub: bool,
    grid: &'a Grid,
    puzzle: &'a Puzzle<C>,
) {
    use std::fmt::Write;
    let mut clues = String::new();

    for clue in clue_lane.clues {
        write!(clues, "{} ", clue.to_string(puzzle)).unwrap();
    }

    let r_or_c = if clue_lane.row { "R" } else { "C" };

    print!("{}{: <3} {: >16}", r_or_c, clue_lane.index, clues);

    if scrub {
        print!(" ! ");
    } else {
        print!(" | ");
    }

    for (orig, now) in orig_lane.iter().zip(get_grid_lane(clue_lane, grid)) {
        let new_ch = match now.known_or() {
            None => "?".to_string(),
            Some(known_color) => puzzle.palette[&known_color].ch.to_string(),
        };

        if *orig != *now {
            print!("{}", new_ch.underline());
        } else {
            print!("{}", new_ch);
        }
    }

    // Hackish way of getting the original score...
    if scrub {
        let lane_arr: ndarray::Array1<Cell> = orig_lane.into();
        let orig_score =
            scrub_heuristic(clue_lane.clues, lane_arr.rows().into_iter().next().unwrap());
        println!("   {}->{}", orig_score, clue_lane.scrub_score);
    } else {
        let lane_arr: ndarray::Array1<Cell> = orig_lane.into();
        let orig_score =
            skim_heuristic(clue_lane.clues, lane_arr.rows().into_iter().next().unwrap());
        println!("   {}->{}", orig_score, clue_lane.skim_score);
    }
}

pub fn solve<C: Clue>(puzzle: &Puzzle<C>, trace_solve: bool) -> anyhow::Result<Report> {
    let mut grid = Grid::from_elem((puzzle.rows.len(), puzzle.cols.len()), Cell::new(puzzle));

    let mut solve_lanes = vec![];

    for (idx, clue_row) in puzzle.rows.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_row, true, idx, &grid));
    }

    for (idx, clue_col) in puzzle.cols.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_col, false, idx, &grid));
    }

    let progress = indicatif::ProgressBar::new_spinner();
    if trace_solve {
        progress.finish_and_clear();
    }

    let mut cells_left = puzzle.rows.len() * puzzle.cols.len();
    let mut skims = 0;
    let mut scrubs = 0;

    let mut allowed_skims = 10;
    loop {
        progress.tick();
        let will_scrub = allowed_skims == 0;

        let (report, was_row) = {
            let best_clue_lane = match find_best_lane(&mut solve_lanes, will_scrub) {
                Some(lane) => lane,
                None => {
                    if will_scrub {
                        print_grid(&grid, puzzle);
                        anyhow::bail!(
                            "Unable to line-solve after {skims} skims and {scrubs} scrubs!"
                        );
                    } else {
                        allowed_skims = 0; // Try again, but scrub.
                        continue;
                    }
                }
            };

            let best_grid_lane = get_mut_grid_lane(best_clue_lane, &mut grid);

            progress.set_message(format!(
                "skims: {skims: >6}  scrubs: {scrubs: >6}  cells left: {cells_left: >6}  skims allowed: {allowed_skims: >3}  {} {}", if will_scrub {
                    "scrubbing".red()
                } else {
                    "skimming".green()
                },
                best_clue_lane.text_coord(),
            ));

            let orig_version_of_line: Vec<Cell> = best_grid_lane.iter().cloned().collect();

            let report = if will_scrub {
                best_clue_lane.scrubbed = true;
                scrubs += 1;
                scrub_line(
                    best_clue_lane.clues,
                    best_grid_lane,
                    best_clue_lane.row,
                    best_clue_lane.index,
                    puzzle,
                )?
            } else {
                best_clue_lane.skimmed = true;
                skims += 1;
                skim_line(
                    best_clue_lane.clues,
                    best_grid_lane,
                    best_clue_lane.row,
                    best_clue_lane.index,
                    puzzle,
                )?
            };

            best_clue_lane.rescore(&grid, /*was_processed=*/ true);

            // TODO: there's got to be a simpler way than calling `get_mut_grid_lane` again.
            // Maybe just have `skim`/`scrub` report the difference directly
            let known_before = orig_version_of_line.iter().filter(|c| c.is_known()).count();
            let known_after = get_mut_grid_lane(best_clue_lane, &mut grid)
                .iter()
                .filter(|c| c.is_known())
                .count();

            cells_left -= known_after - known_before;

            if trace_solve {
                display_step(
                    best_clue_lane,
                    orig_version_of_line,
                    will_scrub,
                    &grid,
                    puzzle,
                );
            }

            (report, best_clue_lane.row)
        };

        if cells_left == 0 {
            progress.finish_and_clear();
            println!("Solved in {skims} skims, {scrubs} scrubs.");
            break;
        }

        if will_scrub {
            if !report.affected_cells.is_empty() {
                allowed_skims = 10;
            }
        } else if report.affected_cells.is_empty() {
            allowed_skims -= 1;
        } else {
            allowed_skims = std::cmp::max(10, allowed_skims + 1);
        }

        // Affected intersecting lanes now may need to be re-examined:
        for other_lane in solve_lanes.iter_mut() {
            if other_lane.row != was_row && report.affected_cells.contains(&other_lane.index) {
                other_lane.rescore(&grid, /*was_processed=*/ false);
                other_lane.skimmed = false;
                other_lane.scrubbed = false;
            }
        }
    }

    // Not printing; we probably already know what it looks like!

    Ok(Report {})
}

#[cfg(test)]
mod tests {
    use crate::puzzle::{Nono, Puzzle, Color, ColorInfo, BACKGROUND};
    use crate::grid_solve::solve; // Use crate::grid_solve::solve to refer to the solve in the parent module
    use std::collections::HashMap;
    // use std::fmt::Write; // Not strictly needed for this test itself.

    #[test]
    fn test_contradiction_error_reporting() {
        let mut palette = HashMap::new();
        palette.insert(BACKGROUND, ColorInfo { ch: '.', name: "background".to_string(), rgb: (255,255,255), color: BACKGROUND, corner: None });
        let color_a = Color(1);
        let color_b = Color(2);
        palette.insert(color_a, ColorInfo { ch: 'A', name: "ColorA".to_string(), rgb: (255,0,0), color: color_a, corner: None });
        palette.insert(color_b, ColorInfo { ch: 'B', name: "ColorB".to_string(), rgb: (0,0,255), color: color_b, corner: None });

        // Puzzle: 1 row, 2 columns
        // Row 1: [A2] (Fill 2 with A)
        // Col 1: [B1] (Fill 1 with B)
        // Col 2: [B1] (Fill 1 with B)
        // Contradiction: Cell (R1,C1) must be A (from row clue) and B (from col clue).
        let puzzle = Puzzle::<Nono> {
            palette,
            rows: vec![
                vec![Nono { color: color_a, count: 2 }],
            ],
            cols: vec![
                vec![Nono { color: color_b, count: 1 }],
                vec![Nono { color: color_b, count: 1 }],
            ],
        };

        match solve(&puzzle, false) {
            Ok(_) => panic!("Expected a contradiction, but puzzle solved."),
            Err(e) => {
                let error_message = e.to_string();
                eprintln!("Caught error: {}", error_message); // For debugging test failures

                // Check for core parts of the ContradictionError message
                assert!(error_message.contains("Contradiction in"), "Error message should indicate a contradiction details.");
                assert!(error_message.contains("Clues:"), "Error message should include clues.");

                // Check for either row or column context.
                // The exact point of detection can vary (e.g., processing R1, or C1).
                // let mentions_row_context = error_message.contains("Row 1") && error_message.contains("A2");
                // let mentions_col_context = error_message.contains("Column 1") && error_message.contains("B1");
                // It could also be Column 2, cell 1, with clue B1, if R1,C1 was B and R1,C2 was A then contradiction.
                // Or Row 1, cell 2 with clue A2 if C1 was B and C2 was B.

                // Let's check if the error message contains the information from one of the possible points of contradiction
                // For R1 (A2), if it tries to set (0,0) or (0,1)
                // For C1 (B1), if it tries to set (0,0)
                // For C2 (B1), if it tries to set (0,1)

                let r1_clue_str = "A2"; // Nono { color: color_a, count: 2 }
                let c1_clue_str = "B1"; // Nono { color: color_b, count: 1 }

                let case1 = error_message.contains("Row 1") && error_message.contains(r1_clue_str); // Contradiction found while processing Row 1
                let case2 = error_message.contains("Column 1") && error_message.contains(c1_clue_str); // Contradiction found while processing Col 1
                let case3 = error_message.contains("Column 2") && error_message.contains(c1_clue_str); // Contradiction found while processing Col 2

                assert!(case1 || case2 || case3, "Error message did not contain expected row/column and clue information. Message: {}", error_message);
            }
        }
    }
}
