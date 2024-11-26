use anyhow::bail;
use ndarray::{ArrayView1, ArrayViewMut1};

use crate::{
    line_solve::{scrub_heuristic, scrub_line, skim_heuristic, skim_line},
    puzzle::{Clue, Color, Puzzle},
};

type Cell = Option<Color>;
type Grid = ndarray::Array2<Cell>;

pub struct Report {}

pub struct LaneState<'a> {
    clues: &'a [Clue], // just convenience, since `row` and `index` suffice to find it again
    row: bool,
    index: ndarray::Ix,
    scrubbed: bool,
    scrub_score: i32,
    skimmed: bool,
    skim_score: i32,
}

impl<'a> LaneState<'a> {
    fn new(clues: &'a [Clue], row: bool, idx: usize, grid: &Grid) -> LaneState<'a> {
        let mut res = LaneState {
            clues,
            row,
            index: idx,
            scrubbed: false,
            scrub_score: 0,
            skimmed: false,
            skim_score: 0,
        };
        res.rescore(grid);
        res
    }
    fn rescore(&mut self, grid: &Grid) {
        let lane = get_grid_lane(self, grid);
        if lane.iter().all(|cell| cell.is_some()) {
            self.scrub_score = std::i32::MIN;
            self.skim_score = std::i32::MIN;
            return;
        }
        self.scrub_score = scrub_heuristic(self.clues, lane);
        self.skim_score = skim_heuristic(self.clues, lane);
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
        if to_scrub {
            if lane.scrubbed {
                continue;
            }
            if lane.scrub_score > best_score {
                best_score = lane.scrub_score;
                res = Some(lane);
            }
        } else {
            if lane.skimmed {
                continue;
            }
            if lane.skim_score > best_score {
                best_score = lane.skim_score;
                res = Some(lane);
            }
        }
    }
    res
}

pub fn solve(puzzle: &Puzzle) -> anyhow::Result<Report> {
    let mut grid = Grid::default((puzzle.rows.len(), puzzle.cols.len()));

    let mut solve_lanes = vec![];

    for (idx, clue_row) in puzzle.rows.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_row, true, idx, &grid));
    }

    for (idx, clue_col) in puzzle.cols.iter().enumerate() {
        solve_lanes.push(LaneState::new(clue_col, false, idx, &grid));
    }

    let mut cells_left = puzzle.rows.len() * puzzle.cols.len();
    let mut skims = 0;
    let mut scrubs = 0;

    let mut allowed_skims = 10;
    loop {
        let will_scrub = allowed_skims == 0;

        let (report, was_row) = {
            let best_clue_lane = match find_best_lane(&mut solve_lanes, will_scrub) {
                Some(lane) => lane,
                None => {
                    if will_scrub {
                        bail!("Cannot solve");
                    } else {
                        allowed_skims = 0; // Try again, but scrub.
                        continue;
                    }
                }
            };

            let best_grid_lane = get_mut_grid_lane(&best_clue_lane, &mut grid);

            print!(
                "{}{}",
                best_clue_lane.index,
                if best_clue_lane.row { "R" } else { "C" }
            );

            (
                if will_scrub {
                    best_clue_lane.scrubbed = true;
                    scrubs += 1;
                    scrub_line(best_clue_lane.clues, best_grid_lane)?
                } else {
                    best_clue_lane.skimmed = true;
                    skims += 1;
                    skim_line(best_clue_lane.clues, best_grid_lane)?
                },
                best_clue_lane.row,
            )
        };

        print!(
            "{}{} ",
            report.affected_cells.len(),
            if will_scrub { "!" } else { "" }
        );

        cells_left -= report.affected_cells.len();
        if cells_left == 0 {
            println!();
            println!("Solved in {skims} skims, {scrubs} scrubs.");
            break;
        }

        if will_scrub {
            if !report.affected_cells.is_empty() {
                allowed_skims = 6;
            }
        } else {
            if report.affected_cells.is_empty() {
                allowed_skims -= 1;
            } else {
                allowed_skims = std::cmp::max(10, allowed_skims + 1);
            }
        }

        for other_lane in solve_lanes.iter_mut() {
            if other_lane.row != was_row && report.affected_cells.contains(&other_lane.index) {
                other_lane.rescore(&grid);
                other_lane.skimmed = false;
                other_lane.scrubbed = false;
            }
        }
    }

    for row in grid.rows() {
        for cell in row {
            match cell {
                None => {
                    print!(" ");
                }
                Some(c) => {
                    print!("{}", c.0);
                }
            }
        }
        println!();
    }

    Ok(Report {})
}
