use std::{fmt::Debug, sync::mpsc, vec};

use anyhow::Context;
use colored::Colorize;
use ndarray::{ArrayView1, ArrayViewMut1};

use crate::{
    line_solve::{scrub_heuristic, scrub_line, skim_heuristic, skim_line, Cell, ScrubReport},
    puzzle::{Clue, Color, Puzzle, Solution, BACKGROUND},
};

type Grid = ndarray::Array2<Cell>;

pub struct Report {
    pub skims: usize,
    pub scrubs: usize,
    pub cells_left: usize,
    pub solution: Solution,
    pub solved_mask: Vec<Vec<bool>>,
}

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

impl<C: Clue> Debug for LaneState<'_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}: {:?}",
            if self.row { "R" } else { "C" },
            self.index + 1,
            self.clues
        )
    }
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

fn grid_to_solved_mask<C: Clue>(grid: &Grid) -> Vec<Vec<bool>> {
    grid.columns()
        .into_iter()
        .map(|col| {
            col.iter()
                .map(|cell| cell.is_known())
                .collect::<Vec<bool>>()
        })
        .collect()
}

fn grid_to_solution<C: Clue>(grid: &Grid, puzzle: &Puzzle<C>) -> Solution {
    let grid = grid
        .columns()
        .into_iter()
        .map(|col| {
            col.iter()
                .map(|cell| cell.known_or().unwrap_or(BACKGROUND))
                .collect::<Vec<Color>>()
        })
        .collect();
    Solution {
        clue_style: C::style(),
        grid,
        palette: puzzle.palette.clone(),
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

pub type LineCache<C> = std::collections::HashMap<(Vec<C>, Vec<u32>), (ScrubReport, Vec<Cell>)>;

fn op_or_cache<'a, C: Clue, F>(
    f: F,
    solve_lane: &LaneState<'a, C>,
    lane: &mut ArrayViewMut1<Cell>,
    cache: &mut Option<LineCache<C>>,
) -> anyhow::Result<ScrubReport>
where
    F: Fn(&[C], &mut ArrayViewMut1<Cell>) -> anyhow::Result<ScrubReport>,
{
    if let Some(cache) = cache {
        let entry = cache.entry((
            solve_lane.clues.to_vec(),
            lane.iter().map(|cell| cell.raw()).collect::<Vec<_>>(),
        ));
        match entry {
            std::collections::hash_map::Entry::Occupied(o) => {
                let (report, new_cells) = o.get();

                for (idx, new_cell) in report.affected_cells.iter().zip(new_cells) {
                    lane[*idx] = *new_cell;
                }

                return Ok(report.clone());
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                let report = f(solve_lane.clues, lane)?;
                let mut cells_to_cache = vec![];

                for idx in &report.affected_cells {
                    cells_to_cache.push(lane[*idx]);
                }

                v.insert((report.clone(), cells_to_cache));
                return Ok(report);
            }
        }
    } else {
        f(solve_lane.clues, lane)
    }
}

pub fn solve<C: Clue>(
    puzzle: &Puzzle<C>,
    line_cache: &mut Option<LineCache<C>>,
    trace_solve: bool,
) -> anyhow::Result<Report> {
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
                        // Nothing left to try; can't solve.
                        return Ok(Report {
                            skims,
                            scrubs,
                            cells_left,
                            solution: grid_to_solution::<C>(&grid, puzzle),
                            solved_mask: grid_to_solved_mask::<C>(&grid),
                        });
                    } else {
                        allowed_skims = 0; // Try again, but scrub.
                        continue;
                    }
                }
            };

            let mut best_grid_lane: ArrayViewMut1<Cell> =
                get_mut_grid_lane(best_clue_lane, &mut grid);

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
                op_or_cache(scrub_line, best_clue_lane, &mut best_grid_lane, line_cache).context(
                    format!(
                        "scrubbing {:?} with {:?}",
                        best_clue_lane, orig_version_of_line
                    ),
                )?
            } else {
                best_clue_lane.skimmed = true;
                skims += 1;
                skim_line(best_clue_lane.clues, &mut best_grid_lane).context(format!(
                    "skimming {:?} with {:?}",
                    best_clue_lane, orig_version_of_line
                ))?
            };

            let known_before = orig_version_of_line.iter().filter(|c| c.is_known()).count();
            let known_after = best_grid_lane.iter().filter(|c| c.is_known()).count();

            best_clue_lane.rescore(&grid, /*was_processed=*/ true);

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
            return Ok(Report {
                skims,
                scrubs,
                cells_left,
                solution: grid_to_solution::<C>(&grid, puzzle),
                solved_mask: grid_to_solved_mask::<C>(&grid),
            });
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
}

pub async fn disambig_candidates(
    s: &Solution,
    progress: mpsc::Sender<f32>,
    terminate: mpsc::Receiver<()>,
) -> Vec<Vec<(Color, f32)>> {
    let mut solve_cache = crate::puzzle::DynSolveCache::new();

    let p = s.to_puzzle();
    // Probably redundant, but a small cost compared to the rest!
    let Report {
        cells_left: orig_cells_left,
        ..
    } = solve_cache
        .solve(&p)
        .expect("started from a solution; shouldn't be possible!");

    let mut res = vec![vec![(BACKGROUND, 0.0); s.grid.first().unwrap().len()]; s.grid.len()];
    if orig_cells_left == 0 {
        // TODO: probably send a result
        progress.send(0.0).unwrap();
        return res;
    }

    for x in 0..s.x_size() {
        for y in 0..s.y_size() {
            let mut best_result = std::usize::MAX;
            let mut best_color = BACKGROUND;

            for new_col in s.palette.keys() {
                if *new_col == s.grid[x][y] {
                    continue;
                }
                let mut new_grid = s.grid.clone();
                new_grid[x][y] = *new_col;
                let new_solution = Solution {
                    grid: new_grid,
                    ..s.clone()
                };

                let Report {
                    cells_left: new_cells_left,
                    ..
                } = solve_cache.solve(&new_solution.to_puzzle()).expect("");

                if new_cells_left < best_result {
                    best_result = new_cells_left;
                    best_color = *new_col;
                }
            }

            if y % 5 == 0 {
                progress
                    .send((x * s.y_size() + y) as f32 / (s.x_size() * s.y_size()) as f32)
                    .unwrap();
            }

            // Works on wasm or native:
            tokio::task::yield_now().await;

            res[x][y] = (best_color, (best_result as f32) / (orig_cells_left as f32));

            if terminate.try_recv().is_ok() {
                return res;
            }
        }
    }
    progress.send(1.0).unwrap();

    return res;
}
