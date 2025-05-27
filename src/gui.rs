// GUI module
use crate::puzzle::{Color, DynPuzzle, BACKGROUND, ColorInfo, Nono, Puzzle}; // Added Nono, Puzzle for export
use crate::grid_solve; // For Report type
use eframe::{egui, App, epaint::{RectShape, Stroke}};
use std::collections::HashMap;
use std::fs; // For file writing

const CELL_SIZE: f32 = 20.0;
const CELL_SPACING: f32 = 1.0;
const PALETTE_CELL_SIZE: f32 = 30.0;
const PALETTE_SPACING: f32 = 4.0;
const SELECTED_BORDER_THICKNESS: f32 = 2.0;

pub struct EditorApp {
    puzzle: DynPuzzle,
    selected_color: Color,
    solve_report: Option<anyhow::Result<grid_solve::Report>>,
    status_message: Option<String>,
}

impl EditorApp {
    pub fn new(mut puzzle: DynPuzzle) -> Self {
        let mut initial_selected_color = BACKGROUND; 

        match &mut puzzle {
            DynPuzzle::Nono(ref mut nono_puzzle) => {
                for (color, _info) in &nono_puzzle.palette {
                    if *color != BACKGROUND {
                        initial_selected_color = *color;
                        break;
                    }
                }
                if initial_selected_color == BACKGROUND {
                    if nono_puzzle.palette.contains_key(&Color(1)) {
                        initial_selected_color = Color(1);
                    } else {
                        if !nono_puzzle.palette.contains_key(&Color(1)) {
                             nono_puzzle.palette.insert(Color(1), ColorInfo {
                                 ch: 'B',
                                 name: "Black".to_string(),
                                 rgb: (0,0,0),
                                 color: Color(1),
                             });
                        }
                        initial_selected_color = Color(1);
                    }
                }

                let num_rows = nono_puzzle.rows.len();
                let num_cols = nono_puzzle.cols.len();

                if num_rows == 0 || num_cols == 0 {
                    nono_puzzle.grid = Vec::new();
                } else if nono_puzzle.grid.is_empty() || 
                          nono_puzzle.grid.len() != num_rows || 
                          nono_puzzle.grid[0].len() != num_cols {
                    nono_puzzle.grid = vec![vec![BACKGROUND; num_cols]; num_rows];
                }
            }
            DynPuzzle::Triano(_) => {
                // Selected color remains BACKGROUND
            }
        }

        Self {
            puzzle,
            selected_color: initial_selected_color,
            solve_report: None,
            status_message: None,
        }
    }
}

impl App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Nonogram Editor");
            
            // Buttons Section
            ui.horizontal(|ui| {
                if ui.button("Export Puzzle").clicked() {
                    self.status_message = None; // Clear previous status
                    self.solve_report = None; // Clear solve report
                    match &self.puzzle {
                        DynPuzzle::Nono(nono_puzzle) => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("WebPBN XML", &["xml"])
                                .set_file_name("puzzle.xml")
                                .save_file()
                            {
                                let export_data = crate::export::as_webpbn(nono_puzzle);
                                match fs::write(&path, export_data) {
                                    Ok(_) => self.status_message = Some(format!("Puzzle exported to {}", path.display())),
                                    Err(e) => self.status_message = Some(format!("Failed to export: {}", e)),
                                }
                            } else {
                                self.status_message = Some("Export cancelled.".to_string());
                            }
                        }
                        DynPuzzle::Triano(_) => {
                            self.status_message = Some("Export currently only supports Nonogram puzzles.".to_string());
                        }
                    }
                }

                if ui.button("Solve Puzzle").clicked() {
                    self.status_message = None; // Clear previous status
                    self.solve_report = Some(self.puzzle.solve(false)); // Use false for no trace_solve
                }
            });
            ui.add_space(5.0);

            // Display Status Message (for export)
            if let Some(msg) = &self.status_message {
                ui.label(msg);
            }

            // Display Solve Report
            if let Some(report_result) = &self.solve_report {
                match report_result {
                    Ok(_report) => { // _report is grid_solve::Report which is currently an empty struct
                        ui.label("Puzzle solved successfully!");
                        // Potentially, if Report struct had data, display it here.
                        // For now, the success of DynPuzzle.solve() means the grid in self.puzzle was updated.
                        // We might need to explicitly copy the solved grid from the report if solve() doesn't mutate.
                        // However, the current `grid_solve::solve` seems to modify the grid in place via `Cell`s.
                        // And `DynPuzzle::solve` calls it. The `Puzzle<Nono>` in `EditorApp`'s `DynPuzzle` has the grid.
                        // So the visual grid should update automatically on the next frame.
                    }
                    Err(e) => {
                        ui.label(format!("Failed to solve puzzle: {}", e));
                    }
                }
            }
            ui.add_space(10.0);


            // Selected Color Display
            let selected_color_info_text = if let DynPuzzle::Nono(ref p) = self.puzzle {
                p.palette.get(&self.selected_color).map_or_else(
                    || format!("ID {} (Unknown)", self.selected_color.0),
                    |info| format!("ID {}, Name: {}, RGB: {:?}", self.selected_color.0, info.name, info.rgb),
                )
            } else {
                format!("ID {} (N/A for Triano)", self.selected_color.0)
            };
            ui.label(format!("Selected color: {}", selected_color_info_text));
            ui.add_space(10.0);

            let puzzle_nono_opt = match &mut self.puzzle {
                DynPuzzle::Nono(p) => Some(p),
                DynPuzzle::Triano(_) => {
                    ui.label("Triano puzzles are not yet supported in the editor.");
                    None
                }
            };
            
            if puzzle_nono_opt.is_none() { return; }
            let puzzle_nono = puzzle_nono_opt.unwrap();


            if puzzle_nono.rows.is_empty() || puzzle_nono.cols.is_empty() {
                ui.label("Puzzle has no rows or columns to display.");
                return;
            }
            
            let num_rows = puzzle_nono.rows.len();
            let num_cols = puzzle_nono.cols.len();

            let grid_width = num_cols as f32 * (CELL_SIZE + CELL_SPACING) - CELL_SPACING;
            let grid_height = num_rows as f32 * (CELL_SIZE + CELL_SPACING) - CELL_SPACING;

            let (response, painter) = ui.allocate_painter(
                egui::vec2(grid_width, grid_height),
                egui::Sense::click(),
            );
            
            let grid_rect = response.rect;

            for r_idx in 0..num_rows {
                for c_idx in 0..num_cols {
                    let cell_top_left = grid_rect.min + 
                        egui::vec2(
                            c_idx as f32 * (CELL_SIZE + CELL_SPACING),
                            r_idx as f32 * (CELL_SIZE + CELL_SPACING),
                        );
                    let cell_rect = egui::Rect::from_min_size(cell_top_left, egui::vec2(CELL_SIZE, CELL_SIZE));
                    
                    let color_id = puzzle_nono.grid[r_idx][c_idx]; // grid is part of Puzzle<Nono>
                    
                    let fill_color = if color_id == BACKGROUND {
                        egui::Color32::LIGHT_GRAY
                    } else {
                        puzzle_nono.palette.get(&color_id)
                            .map_or(egui::Color32::RED, |color_info| { 
                                egui::Color32::from_rgb(color_info.rgb.0, color_info.rgb.1, color_info.rgb.2)
                            })
                    };
                    
                    painter.add(RectShape::filled(cell_rect, egui::Rounding::none(), fill_color));
                }
            }

            if response.clicked() {
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    let relative_pos = mouse_pos - grid_rect.min;
                    
                    let clicked_col = (relative_pos.x / (CELL_SIZE + CELL_SPACING)).floor() as isize;
                    let clicked_row = (relative_pos.y / (CELL_SIZE + CELL_SPACING)).floor() as isize;

                    if clicked_row >= 0 && clicked_row < num_rows as isize && 
                       clicked_col >= 0 && clicked_col < num_cols as isize {
                        let r_idx = clicked_row as usize;
                        let c_idx = clicked_col as usize;
                        puzzle_nono.grid[r_idx][c_idx] = self.selected_color;
                        self.status_message = None; // Clear status on interaction
                        self.solve_report = None; // Clear solve report on interaction
                    }
                }
            }

            ui.add_space(20.0); 
            ui.separator();
            ui.heading("Color Palette");
            ui.add_space(5.0);

            let mut sorted_palette: Vec<_> = puzzle_nono.palette.iter().collect();
            sorted_palette.sort_by_key(|(color_id, _)| color_id.0);
            
            ui.horizontal_wrapped(|ui| {
                for (color_id, color_info) in sorted_palette {
                    let desired_size = egui::vec2(PALETTE_CELL_SIZE, PALETTE_CELL_SIZE);
                    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
                    
                    let fill_color = egui::Color32::from_rgb(color_info.rgb.0, color_info.rgb.1, color_info.rgb.2);
                    ui.painter().rect_filled(rect, egui::Rounding::none(), fill_color);

                    if *color_id == self.selected_color {
                        ui.painter().rect_stroke(
                            rect.expand(SELECTED_BORDER_THICKNESS / 2.0), 
                            egui::Rounding::none(),
                            Stroke::new(SELECTED_BORDER_THICKNESS, egui::Color32::BLACK), 
                        );
                    }
                    
                    if response.clicked() {
                        self.selected_color = *color_id;
                        self.status_message = None; // Clear status on interaction
                        self.solve_report = None; // Clear solve report on interaction
                    }
                    ui.add_space(PALETTE_SPACING); 
                }
            });
        });
    }
}
