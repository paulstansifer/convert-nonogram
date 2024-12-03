// They're used in tests, but it can't see that.
#![allow(unused_macros)]

use std::u32;

use crate::puzzle::{Clue, Color, Puzzle, BACKGROUND};
use anyhow::{bail, Context};
use ndarray::{ArrayView1, ArrayViewMut1};

// type ClueSlice = Vec<Clue>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    possible_color_mask: u32,
}

impl Cell {
    pub fn new(puzzle: &Puzzle) -> Cell {
        let mut res: u32 = 0;
        for color in puzzle.palette.keys() {
            res |= 1 << color.0
        }
        Cell {
            possible_color_mask: res,
        }
    }

    /// Not much practical difference between this and `new`.
    pub fn new_anything() -> Cell {
        Cell {
            possible_color_mask: u32::MAX,
        }
    }

    pub fn from_colors(colors: &[Color]) -> Cell {
        let mut res = Self::new_impossible();
        for c in colors {
            res.actually_could_be(*c);
        }
        res
    }

    pub fn from_color(color: Color) -> Cell {
        Cell {
            possible_color_mask: 1 << color.0,
        }
    }

    pub fn is_known(&self) -> bool {
        self.possible_color_mask.is_power_of_two()
    }

    pub fn is_known_to_be(&self, color: Color) -> bool {
        self.possible_color_mask == 1 << color.0
    }

    pub fn can_be(&self, color: Color) -> bool {
        (self.possible_color_mask & 1 << color.0) != 0
    }

    pub fn known_or(&self) -> Option<Color> {
        if !self.is_known() {
            None
        } else {
            Some(Color(self.possible_color_mask.ilog2() as u8))
        }
    }

    /// Returns whether anything new was discovered (or an error if it's impossible)
    pub fn learn(&mut self, color: Color) -> anyhow::Result<bool> {
        if !self.can_be(color) {
            bail!("learned a contradiction");
        }
        let already_known = self.is_known();
        self.possible_color_mask = 1 << color.0;
        Ok(!already_known)
    }

    pub fn learn_intersect(&mut self, possible: Cell) -> anyhow::Result<bool> {
        if self.possible_color_mask & possible.possible_color_mask == 0 {
            bail!("learned a contradiction");
        }
        let orig_mask = self.possible_color_mask;
        self.possible_color_mask &= possible.possible_color_mask;

        Ok(self.possible_color_mask != orig_mask)
    }

    /// Returns whether anything new was discovered (or an error if it's impossible)
    pub fn learn_that_not(&mut self, color: Color) -> anyhow::Result<bool> {
        if self.is_known_to_be(color) {
            bail!("learned a contradiction");
        }
        let already_known = !self.can_be(color);
        self.possible_color_mask &= !(1 << color.0);
        Ok(!already_known)
    }

    /// Doesn't make sense in the grid, but useful for scrubbing.
    pub fn new_impossible() -> Cell {
        Cell {
            possible_color_mask: 0,
        }
    }

    /// Doesn't make sense in the grid, but useful for scrubbing.
    pub fn actually_could_be(&mut self, color: Color) {
        self.possible_color_mask |= 1 << color.0;
    }

    pub fn contradictory(&self) -> bool {
        self.possible_color_mask == 0
    }

    pub fn unwrap_color(&self) -> Color {
        self.known_or().unwrap()
    }
}

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
                if i == self.gaps.len() - 1 {
                    return None;
                }
                if self.gaps[i] == 0 {
                    continue;
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

fn learn_cell(
    color: Color,
    lane: &mut ArrayViewMut1<Cell>,
    idx: usize,
    affected_cells: &mut Vec<usize>,
) -> anyhow::Result<()> {
    if lane[idx].learn(color)? {
        affected_cells.push(idx);
    }
    Ok(())
}

fn learn_cell_intersect(
    possibilities: Cell,
    lane: &mut ArrayViewMut1<Cell>,
    idx: usize,
    affected_cells: &mut Vec<usize>,
) -> anyhow::Result<()> {
    if lane[idx].learn_intersect(possibilities)? {
        affected_cells.push(idx);
    }
    Ok(())
}

struct ClueAdjIterator<'a> {
    clues: &'a [Clue],
    i: usize,
}
impl<'a> ClueAdjIterator<'a> {
    fn new(clues: &'a [Clue]) -> ClueAdjIterator<'a> {
        ClueAdjIterator { clues: clues, i: 0 }
    }
}

impl<'a> Iterator for ClueAdjIterator<'a> {
    type Item = (bool, &'a Clue, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i == self.clues.len() {
            return None;
        }
        let res = (
            self.i > 0 && self.clues[self.i - 1].color == self.clues[self.i].color,
            &self.clues[self.i],
            self.i < self.clues.len() - 1
                && self.clues[self.i + 1].color == self.clues[self.i].color,
        );
        self.i += 1;
        Some(res)
    }
}

///  For example, (1 2 1) with no other constraints gives
///  .] .  .  .]  .  .]
fn packed_extents(clues: &[Clue], lane: &ArrayViewMut1<Cell>, reversed: bool) -> Vec<usize> {
    let mut extents: Vec<usize> = vec![];

    let lane_at = |idx: usize| -> Cell {
        if reversed {
            lane[lane.len() - 1 - idx]
        } else {
            lane[idx]
        }
    };
    let clue_at = |idx: usize| -> &Clue {
        if reversed {
            &clues[clues.len() - 1 - idx]
        } else {
            &clues[idx]
        }
    };

    // -- Pack to the left (we've abstracted over `reversed`) --

    let mut pos = 0 as usize;
    let mut last_color = None;
    for clue_idx in 0..clues.len() {
        let clue = clue_at(clue_idx);
        if Some(clue.color) == last_color {
            pos += 1;
        }
        // Scanning backwards for mismatches lets us jump farther sometimes.
        let mut placeable = false;
        while !placeable {
            placeable = true;
            for possible_pos in (pos..(pos + clue.count as usize)).rev() {
                // TODO: `possible_pos` can get too high if clues are contradictory; use `Result`
                let cur = lane_at(possible_pos);

                if !cur.can_be(clue.color) {
                    pos = possible_pos + 1;
                    placeable = false;
                    break;
                }
            }
        }
        extents.push(pos + clue.count as usize - 1);
        pos += clue.count as usize;
        last_color = Some(clue.color);
    }

    // TODO: pull out into a separate function!

    // We might be able to do better; are there any orphaned foreground cells off to the right?
    // (so this `.rev()` has nothing to do with `reversed`!)

    let mut cur_extent_idx = extents.len() - 1;
    let mut i = lane.len() - 1;
    loop {
        if !lane_at(i).can_be(BACKGROUND) {
            // We don't check that the affected clue and the cell have the same color!
            // That's okay for this conservative approximation, but also kinda silly.

            // We ought to reel in clues until we get one of the right color, but that's hard.
            // We're also ignoring the effects of known background squares and gaps between blocks /
            // of the same color. Perhaps some kind of recursion is appropriate here!
            if extents[cur_extent_idx] < i {
                // Pull it in!
                extents[cur_extent_idx] = i;
            }
            // Either way, skip past the rest of the postulated foreground cells
            //  and keep looking.

            // Fencepost farm here!
            // Suppose we pulled a clue with width 3 into position 8:
            //  0  1  2  3  4  5  6  7  8  9
            //                   [      #]
            // 8 - 3 = 5 is the next cell we need to examine. But we'll `-= 1` below, so add 1.
            i = extents[cur_extent_idx] + 1 - clue_at(cur_extent_idx).count as usize;
            if cur_extent_idx == 0 {
                break;
            }
            cur_extent_idx -= 1;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }

    // -- oh, but fix up the return value --

    if reversed {
        extents.reverse();
        for extent in extents.iter_mut() {
            *extent = lane.len() - *extent - 1;
        }
    }

    extents
}

pub fn skim_line(clues: &[Clue], mut lane: ArrayViewMut1<Cell>) -> anyhow::Result<ScrubReport> {
    let mut affected = Vec::<usize>::new();
    if clues.is_empty() {
        // Special case, so we can safely take the first and last clue.
        for i in 0..lane.len() {
            learn_cell(BACKGROUND, &mut lane, i, &mut affected)?
        }
        return Ok(ScrubReport {
            affected_cells: affected,
        });
    }

    let left_packed_right_extents = packed_extents(clues, &lane, false);
    let right_packed_left_extents = packed_extents(clues, &lane, true);

    for ((gap_before, clue, gap_after), (left_extent, right_extent)) in ClueAdjIterator::new(clues)
        .zip(
            right_packed_left_extents
                .iter()
                .zip(left_packed_right_extents.iter()),
        )
    {
        for idx in (*left_extent)..=(*right_extent) {
            learn_cell(clue.color, &mut lane, idx, &mut affected).context("overlap")?
        }

        // TODO: this seems to still be necessary, despite the background inference below!
        // Figure out why.
        if (*right_extent as i16 - *left_extent as i16) + 1 == clue.count as i16 {
            if gap_before {
                learn_cell(BACKGROUND, &mut lane, left_extent - 1, &mut affected).context("gb")?
            }
            if gap_after {
                learn_cell(BACKGROUND, &mut lane, right_extent + 1, &mut affected).context("ga")?
            }
        }
    }

    // TODO: `packed_extents` should just return both extents of each block.
    let right_packed_right_extents = right_packed_left_extents
        .iter()
        .zip(clues.iter())
        .map(|(extent, clue)| extent + clue.count as usize - 1);
    let left_packed_left_extents = left_packed_right_extents
        .iter()
        .zip(clues.iter())
        .map(|(extent, clue)| extent + 1 - clue.count as usize);

    // Similarly, are there squares between adjacent blocks that can't be hit (must be background)?
    // I learned you can do this from `pbnsolve`.
    for (right_extent_prev, left_extent) in
        right_packed_right_extents.zip(left_packed_left_extents.skip(1))
    {
        if left_extent == 0 {
            continue;
        }
        for idx in (right_extent_prev + 1)..=(left_extent - 1) {
            learn_cell(BACKGROUND, &mut lane, idx, &mut affected).context("empty between")?
        }
    }

    let leftmost = left_packed_right_extents[0] as i16 - clues[0].count as i16;
    let rightmost =
        right_packed_left_extents.last().unwrap() + clues.last().unwrap().count as usize;

    for i in 0..=leftmost {
        learn_cell(BACKGROUND, &mut lane, i as usize, &mut affected).context("lopen")?
    }
    for i in rightmost..lane.len() {
        learn_cell(BACKGROUND, &mut lane, i, &mut affected).context("ropen")?
    }

    Ok(ScrubReport {
        affected_cells: affected,
    })
}

pub fn skim_heuristic(clues: &[Clue], lane: ArrayView1<Cell>) -> i32 {
    if clues.is_empty() {
        return 1000; // Can solve it right away!
    }
    let mut longest_foregroundable_span = 0;
    let mut cur_foregroundable_span = 0;

    for cell in lane {
        if !cell.is_known_to_be(BACKGROUND) {
            cur_foregroundable_span += 1;
            longest_foregroundable_span =
                std::cmp::max(cur_foregroundable_span, longest_foregroundable_span);
        } else {
            cur_foregroundable_span = 0;
        }
    }

    let total_clue_length = clues.iter().map(|c| c.count).sum::<u16>();

    let longest_clue = clues.iter().map(|c| c.count).max().unwrap();

    let edge_bonus = if !lane.first().unwrap().is_known_to_be(BACKGROUND) {
        2
    } else {
        0
    } + if !lane.last().unwrap().is_known_to_be(BACKGROUND) {
        2
    } else {
        0
    };

    (total_clue_length + longest_clue) as i32 - longest_foregroundable_span + edge_bonus
}

pub fn scrub_line(cs: &[Clue], mut lane: ArrayViewMut1<Cell>) -> anyhow::Result<ScrubReport> {
    let mut possibilities_lane: Vec<Cell> = vec![Cell::new_impossible(); lane.len()];

    let dimension = lane.len() as u16;

    let bg_sq = bg_squares(cs, dimension);

    let mut found_something = false;
    for gaps in PossibleArrangements::new(cs.len() as u16, bg_sq) {
        let mut arrangement_impossible = false;
        for i in 1..cs.len() {
            if cs[i - 1].color == cs[i].color && gaps[i] == 0 {
                // Adjacent blocks of the same color need at least one space of separation
                arrangement_impossible = true;
            }
        }
        if arrangement_impossible {
            continue;
        }

        for (this_color, known_color) in Arrangement::new(cs, &gaps, dimension).zip(lane.iter()) {
            if !known_color.can_be(this_color) {
                arrangement_impossible = true;
            }
        }
        if arrangement_impossible {
            continue;
        }

        found_something = true;

        for (possibility_color, this_color) in possibilities_lane
            .iter_mut()
            .zip(Arrangement::new(cs, &gaps, dimension))
        {
            possibility_color.actually_could_be(this_color);
        }
    }

    if !found_something {
        bail!("No possible arrangements found.")
    }

    let mut res = ScrubReport {
        affected_cells: vec![],
    };

    for (idx, possible_colors) in possibilities_lane.iter().enumerate() {
        if possible_colors.contradictory() {
            bail!("Scrubbing found no arrangements")
        }

        learn_cell_intersect(*possible_colors, &mut lane, idx, &mut res.affected_cells)
            .expect("scrubbing error")
    }

    Ok(res)
}

pub fn scrub_heuristic(clues: &[Clue], lane: ArrayView1<Cell>) -> i32 {
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
        .filter(|cell| cell.is_known_to_be(BACKGROUND))
        .count() as i32;

    let unknown_cells = lane.into_iter().filter(|cell| !cell.is_known()).count() as i32;

    let known_foreground_cells = lane.len() as i32 - unknown_cells - known_background_cells;

    // scrubbing colored squares back and forth is likely to show colored squares if this is high:
    let density = space_taken - known_foreground_cells + longest_clue - clues.len() as i32;

    let mut known_foreground_chunks: i32 = 0;
    let mut in_a_foreground_chunk = false;
    for cell in lane {
        if !cell.can_be(BACKGROUND) {
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

// Uses `Cell` everywhere, even in the clues, for simplicity, even though clues have to be one
// specific_color
macro_rules! t_scrub {
    ([$($color:expr, $count:expr);*] $($state:expr),*) => {
        {
            let mut initial = ndarray::arr1(&[ $($state),* ]);
            scrub_line(
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count} ),* ],
                initial.rows_mut().into_iter().next().unwrap())
                    .expect("impossible!");
            initial
        }
    };
}

macro_rules! t_skim {
    ([$($color:expr, $count:expr);*] $($state:expr),*) => {
        {
            let mut initial = ndarray::arr1(&[ $($state),* ]);
            skim_line(
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count} ),* ],
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
fn scrub_test() {
    let bw = Cell::from_colors(&[BACKGROUND, Color(1)]);
    let w = Cell::from_color(Color(0));
    let b = Cell::from_color(Color(1));

    assert_eq!(t_scrub!([b, 1]  bw, bw, bw, bw), t_line!(bw, bw, bw, bw));

    assert_eq!(t_scrub!([b, 1]  w, bw, bw, bw), t_line!(w, bw, bw, bw));

    assert_eq!(t_scrub!([b, 1; b, 2]  bw, bw, bw, bw), t_line!(b, w, b, b));

    assert_eq!(t_scrub!([b, 1]  bw, bw, b, bw), t_line!(w, w, b, w));

    assert_eq!(t_scrub!([b, 3]  bw, bw, bw, bw), t_line!(bw, b, b, bw));

    assert_eq!(
        t_scrub!([b, 3]  bw, b, bw, bw, bw),
        t_line!(bw, b, b, bw, w)
    );

    assert_eq!(
        t_scrub!([b, 2; b, 2]  bw, bw, bw, bw, bw),
        t_line!(b, b, w, b, b)
    );

    let rbw = Cell::from_colors(&[BACKGROUND, Color(1), Color(2)]);
    let r = Cell::from_color(Color(2));
    let rw = Cell::from_colors(&[BACKGROUND, Color(2)]);
    let bw = Cell::from_colors(&[BACKGROUND, Color(1)]);

    // Different colors don't need separation, so we don't know as much:
    assert_eq!(
        t_scrub!([r, 2; b, 2]  rbw, rbw, rbw, rbw, rbw),
        t_line!(rw, r, rbw, b, bw)
    );
}

#[test]
fn skim_test() {
    let x = Cell::new_anything();
    let w = Cell::from_color(Color(0));
    let b = Cell::from_color(Color(1));
    let r = Cell::from_color(Color(2));

    assert_eq!(t_skim!([b, 1]  x, x, x, x), t_line!(x, x, x, x));

    assert_eq!(t_skim!([b, 1]  w, x, x, x), t_line!(w, x, x, x));

    assert_eq!(t_skim!([b, 3]  x, x, x, x), t_line!(x, b, b, x));

    assert_eq!(t_skim!([b, 2; b, 1]  x, x, x, x), t_line!(b, b, w, b));

    assert_eq!(t_skim!([b, 1; b, 2]  x, x, x, x), t_line!(b, w, b, b));

    assert_eq!(
        t_skim!([b, 2]  x, x, x, x, x, b, b, x),
        t_line!(w, w, w, w, w, b, b, w)
    );

    assert_eq!(t_skim!([b, 1]  x, x, b, x), t_line!(w, w, b, w));

    assert_eq!(t_skim!([b, 3]  x, b, x, x, x), t_line!(x, b, b, x, w));

    assert_eq!(t_skim!([b, 2; b, 2]  x, x, x, x, x), t_line!(b, b, w, b, b));

    // Different colors don't need separation, so we don't know as much:
    assert_eq!(t_skim!([r, 2; b, 2]  x, x, x, x, x), t_line!(x, r, x, b, x));
}

macro_rules! t_heur {
    ([$($color:expr, $count:expr);*] $($state:expr),*) => {
        {
            let initial = ndarray::arr1(&[ $($state),* ]);
            scrub_heuristic(
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count} ),* ],
                initial.rows().into_iter().next().unwrap())
        }
    };
}

#[test]
fn heuristic_examples() {
    let x = Cell::new_anything();
    let w = Cell::from_color(Color(0));
    let b = Cell::from_color(Color(1));

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
