// They're used in tests, but it can't see that.
#![allow(unused_macros, dead_code)]

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

    // TODO: this could be a lot more efficient by using a bitmask as an iterator.
    pub fn can_be_iter(&self) -> impl Iterator<Item = Color> {
        let mut res = vec![];
        for i in 0..32 {
            if self.possible_color_mask & (1 << i) != 0 {
                res.push(Color(i));
            }
        }
        res.into_iter()
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

        let color_to_yield: Color;

        if self.block % 2 == 0 { // Gap block
            let gap_len: u16 = if self.block / 2 == self.gaps.len() {
                // Last gap isn't explicitly represented, effectively infinite but bounded by self.len
                // We just need to yield BACKGROUND until self.overall_pos >= self.len
                self.overall_pos +=1;
                self.pos_in_block +=1; // Conceptually, pos_in_block advances in this "virtual" last gap
                return Some(BACKGROUND);
            } else {
                self.gaps[self.block / 2]
            };

            if gap_len == 0 { // Explicit zero-length gap
                self.block += 1;
                self.pos_in_block = 0;
                return self.next();
            }

            if self.pos_in_block >= gap_len {
                self.block += 1;
                self.pos_in_block = 0;
                return self.next(); // Finished this gap block
            }
            color_to_yield = BACKGROUND;
        } else { // Clue block
            let current_clue_obj = &self.cs[(self.block - 1) / 2];
            let clue_total_length = current_clue_obj.total_length();

            if clue_total_length == 0 {
                self.block += 1;
                self.pos_in_block = 0;
                return self.next(); // Skip zero-length clues
            }

            if self.pos_in_block >= clue_total_length {
                self.block += 1;
                self.pos_in_block = 0;
                return self.next(); // Finished this clue block
            }
            color_to_yield = current_clue_obj.get_color_at_offset(self.pos_in_block);
        }

        self.pos_in_block += 1;
        self.overall_pos += 1;

        Some(color_to_yield)
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
            max_sum,
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

fn learn_cell_not(
    color: Color,
    lane: &mut ArrayViewMut1<Cell>,
    idx: usize,
    affected_cells: &mut Vec<usize>,
) -> anyhow::Result<()> {
    if lane[idx].learn_that_not(color)? {
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
        ClueAdjIterator { clues, i: 0 }
    }
}

impl<'a> Iterator for ClueAdjIterator<'a> {
    type Item = (bool, &'a Clue, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i == self.clues.len() {
            return None;
        }
        let current_clue = &self.clues[self.i];

        let needs_separator_before = if self.i > 0 {
            let prev_clue = &self.clues[self.i - 1];
            (prev_clue.post_cap.is_none() || current_clue.pre_cap.is_none())
                && prev_clue.color == current_clue.color
        } else {
            false
        };

        let needs_separator_after = if self.i < self.clues.len() - 1 {
            let next_clue = &self.clues[self.i + 1];
            (current_clue.post_cap.is_none() || next_clue.pre_cap.is_none())
                && next_clue.color == current_clue.color
        } else {
            false
        };

        let res = (needs_separator_before, current_clue, needs_separator_after);
        self.i += 1;
        Some(res)
    }
}

///  For example, (1 2 1) with no other constraints gives
///  .] .  .  .]  .  .]
fn packed_extents(
    clues: &[Clue],
    lane: &ArrayViewMut1<Cell>,
    reversed: bool,
) -> anyhow::Result<Vec<usize>> {
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

    let mut pos = 0_usize;
    // last_color removed
    for clue_idx in 0..clues.len() {
        let clue = clue_at(clue_idx); // clue is clue_at(clue_idx)

        // 2.a Separator Logic
        if clue_idx > 0 {
            let prev_clue = clue_at(clue_idx - 1);
            let current_clue = clue; // current_clue is clue_at(clue_idx)
            if (prev_clue.post_cap.is_none() || current_clue.pre_cap.is_none()) && prev_clue.color == current_clue.color {
                pos += 1;
            }
        }

        // 2.b Placement Loop
        let total_len = clue.total_length();
        if total_len == 0 {
            continue;
        }

        // Scanning backwards for mismatches lets us jump farther sometimes.
        let mut placeable = false;
        while !placeable {
            placeable = true;
            // Loop for checking individual cells updated to use total_len
            for current_lane_idx in (pos..(pos + total_len as usize)).rev() {
                if current_lane_idx >= lane.len() {
                    anyhow::bail!("impossible clue: current_lane_idx {} exceeds lane length {}", current_lane_idx, lane.len());
                }
                let cur = lane_at(current_lane_idx);
                
                let offset_in_clue = (current_lane_idx - pos) as u16;
                let required_color = clue.get_color_at_offset(offset_in_clue);

                if !cur.can_be(required_color) {
                    pos = current_lane_idx + 1; // pos updated
                    placeable = false;
                    break;
                }
            }
        }
        // 2.c Storing Extents and Advancing Position
        extents.push(pos + total_len as usize - 1); // Use total_len
        pos += total_len as usize; // Use total_len
        // last_color assignment removed
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
            // 2.d Second Part: Use total_length()
            i = extents[cur_extent_idx] + 1 - clue_at(cur_extent_idx).total_length() as usize;
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

    Ok(extents)
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

    let left_packed_right_extents = packed_extents(clues, &lane, false)?;
    let right_packed_left_extents = packed_extents(clues, &lane, true)?;

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
        // The condition now uses clue.total_length()
        if (*right_extent as i16 - *left_extent as i16) + 1 == clue.total_length() as i16 {
            if gap_before {
                // Ensure left_extent > 0 before trying to access left_extent - 1
                if *left_extent > 0 {
                    learn_cell(BACKGROUND, &mut lane, left_extent - 1, &mut affected).context("gb")?;
                }
            }
            if gap_after {
                // Ensure right_extent < lane.len() - 1 before trying to access right_extent + 1
                if *right_extent < lane.len() - 1 {
                    learn_cell(BACKGROUND, &mut lane, right_extent + 1, &mut affected).context("ga")?;
                }
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
    let mut res = ScrubReport {
        affected_cells: vec![],
    };

    for i in 0..lane.len() {
        if lane[i].is_known() {
            continue;
        }

        for color in lane[i].can_be_iter() {
            let mut hypothetical_lane = lane.to_owned();

            hypothetical_lane[i] = Cell::from_color(color);

            match skim_line(cs, hypothetical_lane.view_mut()) {
                Ok(_) => { /* no luck: no contradiction */ }
                Err(_) => {
                    // `color` is impossible here
                    learn_cell_not(color, &mut lane, i, &mut res.affected_cells)
                        .context("scrub")?;
                }
            }
        }
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
    let w = BACKGROUND; // Color(0)
    let r_col = Color(1); // Main Red
    let g_col = Color(2); // Main Green
    let cap_c1 = Color(3); // Cap color 1
    let cap_c2 = Color(4); // Cap color 2

    // Existing test, updated
    let clues1 = vec![
        Clue { color: r_col, count: 2, pre_cap: None, post_cap: None },
        Clue { color: g_col, count: 3, pre_cap: None, post_cap: None },
    ];

    let gaps_1a = vec![0, 0];
    let arr_1a = Arrangement::new(&clues1, &gaps_1a, 10);
    assert_eq!(
        arr_1a.collect::<Vec<_>>(),
        vec![r_col, r_col, g_col, g_col, g_col, w, w, w, w, w]
    );

    let gaps_1b = vec![1, 1];
    let arr_1b = Arrangement::new(&clues1, &gaps_1b, 10);
    assert_eq!(
        arr_1b.collect::<Vec<_>>(),
        vec![w, r_col, r_col, w, g_col, g_col, g_col, w, w, w]
    );

    // New test case 2.a
    let clues2a = vec![
        Clue { color: r_col, count: 1, pre_cap: Some((cap_c1, 1)), post_cap: None },
        Clue { color: g_col, count: 1, pre_cap: None, post_cap: None },
    ];
    let gaps_2a = vec![0,0]; // No gap between clue1 and clue2
    let arr_2a = Arrangement::new(&clues2a, &gaps_2a, 5); // cap_c1, r_col, g_col, w, w
    assert_eq!(
        arr_2a.collect::<Vec<_>>(),
        vec![cap_c1, r_col, g_col, w, w]
    );
    
    // New test case 2.b
    let clues2b = vec![
        Clue { color: r_col, count: 1, pre_cap: None, post_cap: Some((cap_c1, 1)) }, // R cap_c1
        Clue { color: g_col, count: 1, pre_cap: Some((cap_c2, 1)), post_cap: None }, // cap_c2 G
    ];
    let gaps_2b = vec![0]; // No gap specified between them, means they are adjacent if no separator needed by logic
                           // total length: R(1) cap_c1(1) cap_c2(1) G(1) = 4
    let arr_2b = Arrangement::new(&clues2b, &gaps_2b, 4);
    assert_eq!(
        arr_2b.collect::<Vec<_>>(),
        // Expected: R, cap_c1, cap_c2, G. Separator logic is not part of Arrangement iterator.
        // Arrangement just lays out what's in the clue based on total_length.
        vec![r_col, cap_c1, cap_c2, g_col]
    );

    let gaps_2c = vec![1]; // A gap of 1 between them
    let arr_2c = Arrangement::new(&clues2b, &gaps_2c, 5); // R cap_c1 W cap_c2 G
    assert_eq!(
        arr_2c.collect::<Vec<_>>(),
        vec![r_col, cap_c1, w, cap_c2, g_col]
    );
}

// Helper for skim_line_cap_tests
fn make_clue(color: Color, count: u16, pre_cap: Option<(Color, u16)>, post_cap: Option<(Color, u16)>) -> Clue {
    Clue { color, count, pre_cap, post_cap }
}

#[test]
fn skim_line_cap_tests() {
    // Define Colors
    let w_col = BACKGROUND; // Color(0)
    let c1_col = Color(1);
    let c2_col = Color(2);
    let cap_a_col = Color(3);
    let cap_b_col = Color(4);

    // Define Cells
    let x_cell = Cell::new_anything(); // Can be anything
    let w_cell = Cell::from_color(w_col);
    let c1_cell = Cell::from_color(c1_col);
    let c2_cell = Cell::from_color(c2_col);
    let cap_a_cell = Cell::from_color(cap_a_col);
    let cap_b_cell = Cell::from_color(cap_b_col);

    // Test Case 3.1 (Packing with Pre-Cap)
    // Clues: [{ color: C1, count: 2, pre_cap: Some((CAP_A, 1)), post_cap: None }] (CAP_A C1 C1)
    // Lane: [x, x, x, x, x] (length 5)
    // Expected: [x, CAP_A, C1, C1, x]
    let clues_3_1 = vec![make_clue(c1_col, 2, Some((cap_a_col, 1)), None)];
    let mut lane_3_1 = ndarray::arr1(&[x_cell, x_cell, x_cell, x_cell, x_cell]);
    skim_line(&clues_3_1, lane_3_1.view_mut()).expect("3.1 failed");
    assert_eq!(lane_3_1, ndarray::arr1(&[x_cell, cap_a_cell, c1_cell, c1_cell, x_cell]));

    // Test Case 3.2 (Separator: Same Main, No Caps at Interface)
    // Clues: [{ color: C1, count: 1, pre_cap: None, post_cap: None }, { color: C1, count: 1, pre_cap: None, post_cap: None }] (C1, C1)
    // Lane: [x, x, x] (length 3)
    // Expected: [C1, W, C1]
    let clues_3_2 = vec![
        make_clue(c1_col, 1, None, None),
        make_clue(c1_col, 1, None, None),
    ];
    let mut lane_3_2 = ndarray::arr1(&[x_cell, x_cell, x_cell]);
    skim_line(&clues_3_2, lane_3_2.view_mut()).expect("3.2 failed");
    assert_eq!(lane_3_2, ndarray::arr1(&[c1_cell, w_cell, c1_cell]));

    // Test Case 3.3 (No Separator: Same Main, Caps at Interface)
    // Clues: [{ color: C1, count: 1, pre_cap: None, post_cap: Some((CAP_A, 1)) }, { color: C1, count: 1, pre_cap: Some((CAP_A, 1)), post_cap: None }] (C1 CAP_A, CAP_A C1)
    // Lane: [x, x, x, x] (length 4)
    // Expected: [C1, CAP_A, CAP_A, C1]
    let clues_3_3 = vec![
        make_clue(c1_col, 1, None, Some((cap_a_col, 1))),
        make_clue(c1_col, 1, Some((cap_a_col, 1)), None),
    ];
    let mut lane_3_3 = ndarray::arr1(&[x_cell, x_cell, x_cell, x_cell]);
    skim_line(&clues_3_3, lane_3_3.view_mut()).expect("3.3 failed");
    assert_eq!(lane_3_3, ndarray::arr1(&[c1_cell, cap_a_cell, cap_a_cell, c1_cell]));

    // Test Case 3.4 (No Separator: Different Main Colors)
    // Clues: [{ color: C1, count: 1, pre_cap: None, post_cap: None }, { color: C2, count: 1, pre_cap: None, post_cap: None }] (C1, C2)
    // Lane: [x, x] (length 2)
    // Expected: [C1, C2]
    let clues_3_4 = vec![
        make_clue(c1_col, 1, None, None),
        make_clue(c2_col, 1, None, None),
    ];
    let mut lane_3_4 = ndarray::arr1(&[x_cell, x_cell]);
    skim_line(&clues_3_4, lane_3_4.view_mut()).expect("3.4 failed");
    assert_eq!(lane_3_4, ndarray::arr1(&[c1_cell, c2_cell]));

    // Test Case 3.5 (Packing and Separator with Mixed Caps)
    // Clue A: { color: C1, count: 2, pre_cap: Some((CAP_A,1)), post_cap: None } (CAP_A C1 C1)
    // Clue B: { color: C1, count: 1, pre_cap: None, post_cap: Some((CAP_B,1)) } (C1 CAP_B)
    // Clues: [Clue A, Clue B] -> Separator IS required.
    // Lane: [x, x, x, x, x, x] (length 6 for CAP_A C1 C1 W C1 CAP_B)
    // Expected: [CAP_A, C1, C1, W, C1, CAP_B]
    let clues_3_5 = vec![
        make_clue(c1_col, 2, Some((cap_a_col, 1)), None), // CAP_A C1 C1
        make_clue(c1_col, 1, None, Some((cap_b_col, 1))), // C1 CAP_B
    ];
    let mut lane_3_5 = ndarray::arr1(&[x_cell, x_cell, x_cell, x_cell, x_cell, x_cell]);
    skim_line(&clues_3_5, lane_3_5.view_mut()).expect("3.5 failed");
    assert_eq!(lane_3_5, ndarray::arr1(&[cap_a_cell, c1_cell, c1_cell, w_cell, c1_cell, cap_b_cell]));
}

#[test]
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
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count, pre_cap: None, post_cap: None } ),* ],
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
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count, pre_cap: None, post_cap: None } ),* ],
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
    let bw = Cell::from_colors(&[BACKGROUND, Color(1)]); // Black or White
    let w = Cell::from_color(BACKGROUND); // White (Color(0))
    let b = Cell::from_color(Color(1)); // Black
    
    // Existing tests in t_scrub! are already updated due to macro modification.

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

    // New test case for scrub_line with caps (Point 4)
    // Clue: [{color: C1, count: 1, pre_cap: Some((CAP_A, 1)), post_cap: None}] (CAP_A C1)
    // Lane: [cell_not_CAP_A, x, x] (length 3).
    // scrub_line should deduce [cell_not_CAP_A, CAP_A, C1].
    // Define new colors/cells for this specific test if not already covered
    let c1_col = Color(1); // Re-use 'b' for C1 for simplicity if it matches Color(1)
    let cap_a_col = Color(5); // New cap color to avoid conflict with r,g,b,w etc.
    
    let c1_cell = Cell::from_color(c1_col);
    let cap_a_cell = Cell::from_color(cap_a_col);
    let x_cell = Cell::new_anything();

    // cell_not_CAP_A: can be BACKGROUND, C1, C2 (assuming C2=Color(2) exists from other tests)
    // For this test, let's make it not BACKGROUND, not C1, not CAP_A.
    // If BACKGROUND is Color(0), C1 is Color(1), R is Color(2), G is Color(3), CAP_C1 is Color(4)
    // Let C1 for this test be Color(1) (like 'b')
    // Let CAP_A be Color(5)
    let cell_not_cap_a = Cell::from_colors(&[BACKGROUND, c1_col, Color(2), Color(3), Color(4)]);


    let clues_scrub_cap = vec![
        Clue { color: c1_col, count: 1, pre_cap: Some((cap_a_col, 1)), post_cap: None }
    ];
    let mut lane_scrub_cap = ndarray::arr1(&[cell_not_cap_a, x_cell, x_cell]);
    // Expected: [cell_not_cap_a, cap_a_cell, c1_cell]
    // scrub_line will try to set lane_scrub_cap[1] to cap_a_col. If that leads to a contradiction
    // (it won't here), it would mark cap_a_col as impossible for lane_scrub_cap[1].
    // Then it would try to set lane_scrub_cap[1] to something else.
    // The key is that if lane_scrub_cap[0] CANNOT be cap_a_col, and the only way to place the clue
    // starting at index 0 is [cap_a_col, c1_col, ...], then that placement is impossible.
    // If the clue MUST start at index 1 (because index 0 cannot be cap_a_col),
    // then index 1 must be cap_a_col and index 2 must be c1_col.
    
    // In scrub_line, if we try to set lane_scrub_cap[0] = cap_a_col, it's a contradiction because
    // cell_not_cap_a cannot be cap_a_col. So, this placement is impossible.
    // The clue must be placed starting at index 1.
    // If we set lane_scrub_cap[1] = cap_a_col, then lane_scrub_cap[2] = c1_col.
    // This seems like the expected outcome for scrub.
    
    // The t_scrub macro itself calls scrub_line.
    // We need to define the clue list directly for the macro.
    // The macro takes [$($color:expr, $count:expr);*]
    // This means we can't directly use the pre-constructed `clues_scrub_cap` with the macro.
    // So, we'll call scrub_line directly for this specific test case.
    
    scrub_line(&clues_scrub_cap, lane_scrub_cap.view_mut()).expect("Scrub line cap test failed");
    assert_eq!(lane_scrub_cap, ndarray::arr1(&[cell_not_cap_a, cap_a_cell, c1_cell]));

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
                &vec![ $( Clue { color: $color.unwrap_color(), count: $count, pre_cap: None, post_cap: None } ),* ],
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
