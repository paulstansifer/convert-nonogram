use ndarray::{ArrayView1, ArrayViewMut1};

use crate::{
    line_solve::{line_quality_heuristic, scrub_line},
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
    score: i32,
}

impl<'a> LaneState<'a> {
    fn rescore(&mut self, grid: &Grid) {
        let lane = get_grid_lane(self, grid);
        self.score = line_quality_heuristic(self.clues, lane);
    }
}

impl std::cmp::PartialEq for LaneState<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.scrubbed == other.scrubbed && self.score == other.score
    }
}

impl std::cmp::Eq for LaneState<'_> {}

impl std::cmp::PartialOrd for LaneState<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for LaneState<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.scrubbed == other.scrubbed {
            self.score.cmp(&other.score)
        } else {
            // scrubbed is low-scoring (useless to look at)!
            self.scrubbed.cmp(&other.scrubbed).reverse()
        }
    }
}

// Pick the highest-scoring unscrubbed lane, and scrub it:
//   * if successful
//     * re-score and unscrub affected lanes
//     * sort again
//

pub fn get_mut_grid_lane<'a>(ls: &LaneState, grid: &'a mut Grid) -> ArrayViewMut1<'a, Cell> {
    if ls.row {
        grid.row_mut(ls.index)
    } else {
        grid.column_mut(ls.index)
    }
}

pub fn get_grid_lane<'a>(ls: &LaneState, grid: &'a Grid) -> ArrayView1<'a, Cell> {
    if ls.row {
        grid.row(ls.index)
    } else {
        grid.column(ls.index)
    }
}

pub fn solve(puzzle: &Puzzle) -> anyhow::Result<Report> {
    let mut grid = Grid::default((puzzle.rows.len(), puzzle.cols.len()));

    let mut solve_lanes = vec![];

    for (idx, clue_row) in puzzle.rows.iter().enumerate() {
        let mut ls = LaneState {
            clues: clue_row,
            row: true,
            index: idx,
            scrubbed: false,
            score: 0,
        };
        ls.rescore(&grid);
        solve_lanes.push(ls)
    }

    for (idx, clue_col) in puzzle.cols.iter().enumerate() {
        let mut ls = LaneState {
            clues: clue_col,
            row: false,
            index: idx,
            scrubbed: false,
            score: 0,
        };
        ls.rescore(&grid);
        solve_lanes.push(ls)
    }

    let mut cells_left = puzzle.rows.len() * puzzle.cols.len();
    let mut scrubs = 0;
    loop {
        solve_lanes.sort();

        // We'll put it back, but we need to separate it to mutate the rest of the vector
        let mut best_lane = solve_lanes.pop().unwrap();

        if best_lane.scrubbed {
            println!("\nCan't solve!");
            break;
        }

        let scrub_report = scrub_line(best_lane.clues, get_mut_grid_lane(&best_lane, &mut grid))?;
        scrubs += 1;
        best_lane.scrubbed = true;

        print!("{} ", scrub_report.affected_cells.len());
        if scrubs % 100 == 0 {
            println!();
        }
        if !scrub_report.affected_cells.is_empty() {
            cells_left -= scrub_report.affected_cells.len();

            if cells_left == 0 {
                println!("\nSolved in {scrubs} scrubs!");
                break;
            }

            for other_lane in solve_lanes.iter_mut() {
                if other_lane.row != best_lane.row
                    && scrub_report.affected_cells.contains(&other_lane.index)
                {
                    other_lane.rescore(&grid);
                    other_lane.scrubbed = false;
                }
            }
        }

        solve_lanes.push(best_lane);
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
