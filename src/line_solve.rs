// They're used in tests, but it can't see that.
#![allow(unused_macros)]
use crate::puzzle::{Clue, Color, BACKGROUND};
use anyhow::bail;
use ndarray::{ArrayView1, ArrayViewMut1};

// type ClueSlice = Vec<Clue>;

// None is "I don't know yet"
type Cell = Option<Color>;

struct Arrangement<'a> {
    cs: &'a [Clue],
    gaps: &'a [u16],
    len: u16,

    block: usize,
    pos_in_block: u16,
    overall_pos: u16,
}

impl<'a> Arrangement<'a> {
    fn new(cs: &'a [Clue], gaps: &'a [u16], len: u16) -> Arrangement<'a> {
        Arrangement {
            cs,
            gaps,
            len,
            block: 0,
            pos_in_block: 0,
            overall_pos: 0,
        }
    }
}

// Arrangement is itself an iterator that produces the colors of the line.
impl Iterator for Arrangement<'_> {
    type Item = Color;

    fn next(&mut self) -> Option<Self::Item> {
        if self.overall_pos >= self.len {
            return None;
        }

        let clue: Clue = if self.block % 2 == 0 {
            // In a gap
            if self.block / 2 == self.gaps.len() {
                // Last gap isn't explicitly represented!
                self.pos_in_block += 1;
                self.overall_pos += 1;
                return Some(BACKGROUND);
            } else {
                // dummy clue for explicit background
                Clue {
                    color: BACKGROUND,
                    count: self.gaps[self.block / 2],
                }
            }
        } else {
            self.cs[(self.block - 1) / 2]
        };

        if self.pos_in_block >= clue.count {
            self.block += 1;
            self.pos_in_block = 0;
            return self.next(); // Oops, we were off the end!
        }
        self.pos_in_block += 1;
        self.overall_pos += 1;

        Some(clue.color)
    }
}

struct PossibleArrangements {
    gaps: Vec<u16>,
    max_sum: u16,
    first_step: bool,
}

impl PossibleArrangements {
    fn new(len: u16, max_sum: u16) -> PossibleArrangements {
        PossibleArrangements {
            gaps: vec![0; len as usize],
            max_sum: max_sum,
            first_step: true,
        }
    }
}

impl Iterator for PossibleArrangements {
    type Item = Vec<u16>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.first_step {
            self.first_step = false;
            return Some(self.gaps.to_owned()); // HACK! Surely there's a better way
        }

        let mut sum: u16 = 0;
        for gap in self.gaps.iter() {
            sum += *gap;
        }
        if sum < self.max_sum {
            self.gaps[0] += 1;
        } else {
            for i in 0..self.gaps.len() {
                if self.gaps[i] == 0 {
                    continue;
                }
                if i == self.gaps.len() - 1 {
                    return None;
                }
                self.gaps[i] = 0;
                self.gaps[i + 1] += 1;
                break;
            }
        }

        Some(self.gaps.to_owned())
    }
}

fn bg_squares(cs: &[Clue], len: u16) -> u16 {
    let mut remaining = len;
    for c in cs {
        remaining -= c.count;
    }
    remaining
}

pub struct ScrubReport {
    pub affected_cells: Vec<usize>,
}

pub fn scrub_line(cs: &[Clue], mut lane: ArrayViewMut1<Cell>) -> anyhow::Result<ScrubReport> {
    let mut scratch_lane: Vec<Cell> = vec![]; // empty for "haven't found a valid arrangement yet"

    let dimension = lane.len() as u16;

    let bg_sq = bg_squares(cs, dimension);

    for gaps in PossibleArrangements::new(cs.len() as u16, bg_sq) {
        let mut contradiction = false;
        for i in 1..cs.len() {
            if cs[i - 1].color == cs[i].color && gaps[i] == 0 {
                // Adjacent blocks of the same color need at least one space of separation
                contradiction = true;
            }
        }
        if contradiction {
            continue;
        }

        for (this_color, known_color) in Arrangement::new(cs, &gaps, dimension).zip(lane.iter()) {
            if let Some(known_color) = known_color {
                if *known_color != this_color {
                    contradiction = true;
                }
            }
        }
        if contradiction {
            continue;
        }

        if scratch_lane.is_empty() {
            // Initialize with the first possible arrangement.
            scratch_lane = Arrangement::new(cs, &gaps, dimension).map(Some).collect();
        } else {
            for (scratch_color, this_color) in scratch_lane
                .iter_mut()
                .zip(Arrangement::new(cs, &gaps, dimension))
            {
                // We've seen a possible difference; we don't know anything about this cell.
                if *scratch_color != Some(this_color) {
                    *scratch_color = None;
                }
            }
        }
    }

    if scratch_lane.is_empty() {
        bail!("Clues are not consistent with what we already know!");
    }

    let mut res = ScrubReport {
        affected_cells: vec![],
    };

    for (idx, (cell, new_knowledge)) in lane.iter_mut().zip(scratch_lane.iter()).enumerate() {
        if cell.is_none() && new_knowledge.is_some() {
            *cell = *new_knowledge;

            res.affected_cells.push(idx)
        } else if cell.is_some() && *cell != *new_knowledge {
            panic!("Shouldn't be possible!");
        }
    }

    Ok(res)
}

pub fn line_quality_heuristic(clues: &[Clue], lane: ArrayView1<Cell>) -> i32 {
    let mut foreground_cells: i32 = 0;
    // If `space_taken == lane.len()`, the line is immediately solvable with no other knowledge.
    let mut space_taken: i32 = 0;
    let mut longest_clue: i32 = 0;
    let mut last_color = None;
    for c in clues {
        foreground_cells += c.count as i32;
        space_taken += c.count as i32;
        if last_color == Some(c.color) {
            space_taken += 1;
        }

        longest_clue = std::cmp::max(longest_clue, c.count as i32);
        last_color = Some(c.color);
    }
    let longest_clue = longest_clue;
    let space_taken = space_taken;

    let known_background_cells = lane
        .into_iter()
        .filter(|cell| **cell == Some(BACKGROUND))
        .count() as i32;

    let unknown_cells = lane.into_iter().filter(|cell| cell.is_none()).count() as i32;

    let known_foreground_cells = lane.len() as i32 - unknown_cells - known_background_cells;

    // scrubbing colored squares back and forth is likely to show colored squares if this is high:
    let density = space_taken - known_foreground_cells + longest_clue - clues.len() as i32;

    let mut known_foreground_chunks: i32 = 0;
    let mut in_a_foreground_chunk = false;
    for cell in lane {
        if cell.is_some() && *cell != Some(BACKGROUND) {
            if !in_a_foreground_chunk {
                known_foreground_chunks += 1;
            }
            in_a_foreground_chunk = true;
        } else {
            in_a_foreground_chunk = false;
        }
    }

    let unknown_background_cells = (lane.len() as i32 - foreground_cells) - known_background_cells;

    // Matching contiguous foreground cells to clues is likely to show background squares if this
    // is high:
    // > 0 is very good, 0 is still good, -1 is alright, -2 is probably not worth looking at.
    let excess_chunks = if known_foreground_cells > 0 {
        known_foreground_chunks - clues.len() as i32
    } else {
        -2
    };

    density + std::cmp::max(0, unknown_background_cells * (excess_chunks + 2) / 2)
}

#[test]
fn arrangement_test() {
    let w = BACKGROUND;
    let r = Color(1);
    let g = Color(2);

    let clues = vec![Clue { color: r, count: 2 }, Clue { color: g, count: 3 }];

    let gaps_1 = vec![0, 0];
    let arr_1 = Arrangement::new(&clues, &gaps_1, 10);
    assert_eq!(
        arr_1.collect::<Vec<_>>(),
        vec![r, r, g, g, g, w, w, w, w, w]
    );

    let gaps_1 = vec![1, 1];
    let arr_1 = Arrangement::new(&clues, &gaps_1, 10);
    assert_eq!(
        arr_1.collect::<Vec<_>>(),
        vec![w, r, r, w, g, g, g, w, w, w]
    );
}

#[test]
fn arrange_gaps_test() {
    {
        assert_eq!(
            PossibleArrangements::new(3, 1).collect::<Vec<_>>(),
            vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 1, 0], vec![0, 0, 1]]
        );
    }

    {
        assert_eq!(
            PossibleArrangements::new(3, 2).collect::<Vec<_>>(),
            vec![
                vec![0, 0, 0],
                vec![1, 0, 0],
                vec![2, 0, 0],
                vec![0, 1, 0],
                vec![1, 1, 0],
                vec![0, 2, 0],
                vec![0, 0, 1],
                vec![1, 0, 1],
                vec![0, 1, 1],
                vec![0, 0, 2]
            ]
        );
    }
}

// Uses `Option<Color>` everywhere, even in the clues, for simplicity, even though `None` is
// invalid there.
macro_rules! t_solve {
    ([$($color:expr, $count:expr);*] $($state:expr),*) => {
        {
            let mut initial = ndarray::arr1(&[ $($state),* ]);
            scrub_line(
                &vec![ $( Clue { color: $color.unwrap(), count: $count} ),* ],
                initial.rows_mut().into_iter().next().unwrap())
                    .expect("impossible!");
            initial
        }
    };
}

macro_rules! t_line {
    ($($new_state:expr),*) => {
        ndarray::arr1(&[ $($new_state),* ])
    };
}

#[test]
fn solve_test() {
    let x = None;
    let w = Some(Color(0));
    let b = Some(Color(1));
    let r = Some(Color(2));

    assert_eq!(t_solve!([b, 1]  x, x, x, x), t_line!(x, x, x, x));

    assert_eq!(t_solve!([b, 1]  w, x, x, x), t_line!(w, x, x, x));

    assert_eq!(t_solve!([b, 1; b, 2]  x, x, x, x), t_line!(b, w, b, b));

    assert_eq!(t_solve!([b, 1]  x, x, b, x), t_line!(w, w, b, w));

    assert_eq!(t_solve!([b, 3]  x, x, x, x), t_line!(x, b, b, x));

    assert_eq!(t_solve!([b, 3]  x, b, x, x, x), t_line!(x, b, b, x, w));

    assert_eq!(
        t_solve!([b, 2; b, 2]  x, x, x, x, x),
        t_line!(b, b, w, b, b)
    );

    // Different colors don't need separation, so we don't know as much:
    assert_eq!(
        t_solve!([r, 2; b, 2]  x, x, x, x, x),
        t_line!(x, r, x, b, x)
    );
}

macro_rules! t_heur {
    ([$($color:expr, $count:expr);*] $($state:expr),*) => {
        {
            let initial = ndarray::arr1(&[ $($state),* ]);
            line_quality_heuristic(
                &vec![ $( Clue { color: $color.unwrap(), count: $count} ),* ],
                initial.rows().into_iter().next().unwrap())
        }
    };
}

#[test]
fn heuristic_examples() {
    let x = None;
    let w = Some(Color(0));
    let b = Some(Color(1));

    assert_eq!(t_heur!([b, 1]  x, x, x, x), 1);
    assert_eq!(t_heur!([b, 1]  w, x, x, x), 1);
    assert_eq!(t_heur!([b, 2]  w, w, x, x), 3);
    assert_eq!(t_heur!([b, 1; b, 2]  x, x, x, x), 4);
    assert_eq!(t_heur!([b, 1]  x, x, b, x), 3);
    assert_eq!(t_heur!([b, 3]  x, x, x, x), 5);
    assert_eq!(t_heur!([b, 3]  x, b, x, x, x), 6);

    assert_eq!(
        t_heur!([b, 10]  x, x, x, x, x, x, x, x, x, x, x, x, x, x, x),
        19
    );
    assert_eq!(
        t_heur!([b, 3]  x, x, x, x, x, x, x, x, x, x, x, x, x, x, x),
        5
    );
    assert_eq!(
        t_heur!([b, 3]  x, x, x, x, b, x, x, x, x, x, x, x, x, x, x),
        16
    );
}
