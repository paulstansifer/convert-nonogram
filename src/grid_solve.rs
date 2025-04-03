use colored::Colorize;
use ndarray::{ArrayView1, ArrayViewMut1};

use crate::{
    line_solve::{scrub_heuristic, scrub_line, skim_heuristic, skim_line, Cell},
    puzzle::{Clue, Puzzle},
};

type Grid = ndarray::Array2<Cell>;

pub struct Report {}

pub struct LaneState<'a> {
    clues: &'a [Clue], // just convenience, since `row` and `index` suffice to find it again
    row: bool,
    index: ndarray::Ix,
    scrubbed: bool,
    scrub_score: i32,
    processed_scrub_score: i32,
    skimmed: bool,
    skim_score: i32,
    processed_skim_score: i32,
}

impl<'a> LaneState<'a> {
    pub fn text_coord(&self) -> String {
        format!("{}{}", if self.row { "R" } else { "C" }, self.index + 1)
    }

    fn new(clues: &'a [Clue], row: bool, idx: usize, grid: &Grid) -> LaneState<'a> {
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

impl std::cmp::PartialEq for LaneState<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.scrubbed == other.scrubbed && self.scrub_score == other.scrub_score
    }
}

impl std::cmp::Eq for LaneState<'_> {}

fn get_mut_grid_lane<'a>(ls: &LaneState, grid: &'a mut Grid) -> ArrayViewMut1<'a, Cell> {
    if ls.row {
        grid.row_mut(ls.index)
    } else {
        grid.column_mut(ls.index)
    }
}

fn get_grid_lane<'a>(ls: &LaneState, grid: &'a Grid) -> ArrayView1<'a, Cell> {
    if ls.row {
        grid.row(ls.index)
    } else {
        grid.column(ls.index)
    }
}

fn find_best_lane<'a, 'b>(
    lanes: &'b mut [LaneState<'a>],
    to_scrub: bool,
) -> Option<&'b mut LaneState<'a>> {
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

fn print_grid(grid: &Grid, puzzle: &Puzzle) {
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

fn display_step<'a>(
    clue_lane: &'a LaneState<'a>,
    orig_lane: Vec<Cell>,
    scrub: bool,
    grid: &'a Grid,
    puzzle: &'a Puzzle,
) {
    use std::fmt::Write;
    let mut clues = String::new();

    for clue in clue_lane.clues {
        write!(clues, "{}{} ", clue.count, puzzle.palette[&clue.color].ch).unwrap();
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

pub fn solve(puzzle: &Puzzle, trace_solve: bool) -> anyhow::Result<Report> {
    let mut grid = Grid::from_elem((puzzle.rows.len(), puzzle.cols.len()), Cell::new(puzzle));

    let mut solve_lanes = vec![];

    for (idx, clue_row) in puzzle.rows.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_row, true, idx, &grid));
    }

    for (idx, clue_col) in puzzle.cols.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_col, false, idx, &grid));
    }

    let progress = indicatif::ProgressBar::new_spinner();

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
                scrub_line(best_clue_lane.clues, best_grid_lane)?
            } else {
                best_clue_lane.skimmed = true;
                skims += 1;
                skim_line(best_clue_lane.clues, best_grid_lane)?
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
