#![allow(dead_code)] // Otherwise, anything not tested by this becomes a warning!

mod export;
mod grid_solve;
mod gui;
mod import;
mod line_solve;
mod puzzle;

#[cfg(test)]
mod tests {
    use crate::import::solution_to_puzzle;
    use crate::line_solve::{scrub_line, skim_line, Cell};
    use crate::puzzle::{Color, Solution, BACKGROUND};
    use ndarray::Array1;
    use rand::{Rng, SeedableRng};

    fn generate_random_line(length: usize, num_colors: u8) -> Vec<Color> {
        let mut rng = rand::thread_rng();
        let mut line = Vec::with_capacity(length);
        let mut current_color = if rng.gen_bool(0.5) {
            BACKGROUND
        } else {
            Color(rng.gen_range(1..=num_colors))
        };
        let mut current_run_length = 0;

        for _ in 0..length {
            if current_run_length == 0 {
                let previous_color = current_color;
                // Make consecutive runs have different colors!
                while current_color == previous_color {
                    current_color = if rng.gen_bool(0.5) {
                        BACKGROUND
                    } else {
                        Color(rng.gen_range(1..=num_colors))
                    };
                }
                current_run_length = rng.gen_range(1..=(length / 2).max(1));
            }
            line.push(current_color);
            current_run_length -= 1;
        }
        line
    }

    fn generate_consistent_partial_solution(
        solution_line: &[Color],
        num_colors: u8,
    ) -> Array1<Cell> {
        let mut rng = rand::thread_rng();
        let mut partial_solution = Vec::with_capacity(solution_line.len());

        for &actual_color in solution_line {
            let mut cell = Cell::new_impossible();
            cell.actually_could_be(actual_color); // Must allow the actual color

            // For other colors, 75% chance of also allowing it
            for i in 0..num_colors {
                let other_color = Color(i);
                if other_color != actual_color && rng.gen_bool(0.75) {
                    cell.actually_could_be(other_color);
                }
            }
            partial_solution.push(cell);
        }
        Array1::from(partial_solution)
    }

    #[test]
    fn fuzzer() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let num_fuzz_cases = 1000;
        let max_line_length = 25;

        for i in 0..num_fuzz_cases {
            for max_colors in 2..=5 {
                let line_length = rng.gen_range(1..=max_line_length);
                let solution_line = generate_random_line(line_length, max_colors);

                // Create a dummy Solution struct to use solution_to_puzzle
                let mut grid = vec![vec![BACKGROUND]; line_length];
                for (j, color) in solution_line.iter().enumerate() {
                    grid[j][0] = *color;
                }
                let dummy_solution = Solution {
                    palette: (0..=max_colors)
                        .map(|i| {
                            let color = Color(i);
                            (
                                color,
                                crate::puzzle::ColorInfo {
                                    ch: ' ', // Dummy char
                                    name: format!("color_{}", i),
                                    rgb: (0, 0, 0), // Dummy rgb
                                    color,
                                    corner: None,
                                },
                            )
                        })
                        .collect(),
                    grid,
                };

                let puzzle = solution_to_puzzle(&dummy_solution);
                let clues = &puzzle.rows[0]; // Get clues for the generated line

                let original_partial_solution =
                    generate_consistent_partial_solution(&solution_line, max_colors);
                let mut sc_partial_solution = original_partial_solution.clone();
                let mut sk_partial_solution = original_partial_solution.clone();

                match skim_line(clues, sk_partial_solution.view_mut()) {
                    Ok(_) => {
                        // Check for inconsistencies
                        for j in 0..line_length {
                            if !sk_partial_solution[j].can_be(solution_line[j]) {
                                panic!(
                                "Fuzz case {}: skim_line deduced inconsistency at index {}.  Clues: {:?}. Original line: {:?}, Partial solution before skim: {:?}, Partial solution after skim: {:?}",
                                i, j, clues, solution_line, original_partial_solution, sk_partial_solution
                            );
                            }
                        }
                    }
                    Err(e) => {
                        // Check for solver-identified inconsistencies.
                        panic!(
                        "Fuzz case {}: skim_line returned an error: {}. Original line: {:?}, Partial solution before skim: {:?}",
                        i, e, solution_line, original_partial_solution
                    );
                    }
                }

                match scrub_line(clues, sc_partial_solution.view_mut()) {
                    Ok(_) => {
                        // Check for inconsistencies
                        for j in 0..line_length {
                            if !sc_partial_solution[j].can_be(solution_line[j]) {
                                panic!(
                                "Fuzz case {}: scrub_line deduced inconsistency at index {}.  Clues: {:?}. Original line: {:?}, Partial solution before skim: {:?}, Partial solution after skim: {:?}",
                                i, j, clues, solution_line, original_partial_solution, sc_partial_solution
                            );
                            }
                        }
                    }
                    Err(e) => {
                        // Check for solver-identified inconsistencies.
                        panic!(
                        "Fuzz case {}: scrub_line returned an error: {}. Original line: {:?}, Partial solution before skim: {:?}",
                        i, e, solution_line, original_partial_solution
                    );
                    }
                }
            }
        }
    }
}
