use egui::{Color32, Frame, Pos2, Rect, RichText, Shape, Style, Vec2, Visuals};

use crate::{
    grid_solve,
    import::{solution_to_puzzle, solution_to_triano_puzzle},
    puzzle::{Clue, Color, ColorInfo, Corner, Nono, Solution, Triano, BACKGROUND},
    ClueStyle,
};

struct NonogramGui {
    picture: Solution,
    current_color: Color,
    scale: f32,
    clue_style: ClueStyle,

    undo_stack: Vec<UndoAction>,

    auto_solve: bool,
    solve_report: String,
    report_stale: bool,
    solved_mask: Vec<Vec<bool>>,
}

#[derive(Clone, Copy, Debug)]
enum UndoAction {
    ChangeColor {
        x: usize,
        y: usize,
        new_color: Color,
    },
}

impl NonogramGui {
    fn new(cc: &eframe::CreationContext<'_>, picture: Solution, clue_style: ClueStyle) -> Self {
        let solved_mask = vec![vec![false; picture.grid[0].len()]; picture.grid.len()];

        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        NonogramGui {
            picture,
            current_color: BACKGROUND,
            scale: 10.0,
            clue_style,

            undo_stack: vec![],

            auto_solve: false,
            solve_report: "".to_string(),
            report_stale: true,
            solved_mask,
        }
    }
}

fn cell_shape(
    ci: &ColorInfo,
    solved: bool,
    x: usize,
    y: usize,
    to_screen: &egui::emath::RectTransform,
) -> Vec<egui::Shape> {
    let (r, g, b) = ci.rgb;
    let color = egui::Color32::from_rgb(r, g, b);

    let actual_cell = match ci.corner {
        None => egui::Shape::rect_filled(
            Rect::from_min_size(to_screen * Pos2::new(x as f32, y as f32), to_screen.scale()),
            0.0,
            color,
        ),
        Some(Corner { left, upper }) => {
            let mut points = vec![];
            // The `+`ed offsets are empirircally-set to make things fit better.
            if left || upper {
                points.push(to_screen * Pos2::new(x as f32, y as f32) + Vec2::new(0.25, -0.5));
            }
            if !left || upper {
                points
                    .push(to_screen * Pos2::new((x + 1) as f32, y as f32) + Vec2::new(0.25, -0.5));
            }
            if !left || !upper {
                points.push(
                    to_screen * Pos2::new((x + 1) as f32, (y + 1) as f32) + Vec2::new(0.25, 0.5),
                );
            }
            if left || !upper {
                points.push(to_screen * Pos2::new(x as f32, (y + 1) as f32) + Vec2::new(0.25, 0.5));
            }

            Shape::convex_polygon(points, color, (0.0, color))
        }
    };

    let mut res = vec![actual_cell];

    if !solved {
        res.push(egui::Shape::circle_filled(
            to_screen * Pos2::new(x as f32 + 0.5, y as f32 + 0.5),
            to_screen.scale().x * 0.3,
            egui::Color32::from_rgb(128, 128, 128),
        ))
    }

    res
}

impl NonogramGui {
    fn palette_editor(&mut self, ui: &mut egui::Ui) {
        let mut picked_color = self.current_color;
        let mut new_color_rgb = None;

        for (color, color_info) in &self.picture.palette {
            let (r, g, b) = &color_info.rgb;

            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(if color_info.corner.is_some() {
                            color_info.ch.to_string()
                        } else {
                            "■".to_string()
                        })
                        .monospace()
                        .color(egui::Color32::from_rgb(*r, *g, *b)),
                    )
                    .clicked()
                {
                    picked_color = *color;
                };
                let mut edited_color = [*r as f32 / 256.0, *g as f32 / 256.0, *b as f32 / 256.0];

                if ui
                    .color_edit_button_rgb(&mut edited_color)
                    .on_hover_text("Click to change color")
                    .changed()
                {
                    picked_color = *color;
                    new_color_rgb = Some(edited_color);
                }
            });
        }

        self.current_color = picked_color;
        if let Some(new_color_rgb) = new_color_rgb {
            self.picture
                .palette
                .entry(picked_color)
                .and_modify(|color_info| {
                    color_info.rgb = (
                        (new_color_rgb[0] * 256.0) as u8,
                        (new_color_rgb[1] * 256.0) as u8,
                        (new_color_rgb[2] * 256.0) as u8,
                    );
                });
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let x_size = self.picture.grid.len();
        let y_size = self.picture.grid.first().unwrap().len();

        Frame::canvas(ui.style()).show(ui, |ui| {
            let (mut response, painter) = ui.allocate_painter(
                egui::Vec2::new(self.scale * x_size as f32, self.scale * y_size as f32),
                egui::Sense::click_and_drag(),
            );

            let to_screen = egui::emath::RectTransform::from_to(
                Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(
                        self.picture.grid.len() as f32,
                        self.picture.grid.first().unwrap().len() as f32,
                    ),
                ),
                response.rect,
            );
            let from_screen = to_screen.inverse();

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if response.clicked() {
                    let canvas_pos = from_screen * pointer_pos;
                    let x = canvas_pos.x as usize;
                    let y = canvas_pos.y as usize;

                    if (0..x_size).contains(&x) && (0..y_size).contains(&y) {
                        if self.picture.grid[x][y] != self.current_color {
                            self.picture.grid[x][y] = self.current_color;
                        } else {
                            self.picture.grid[x][y] = BACKGROUND;
                        }
                    }
                    self.report_stale = true;
                }
            }

            let mut shapes = vec![];

            for y in 0..y_size {
                for x in 0..x_size {
                    let cell = self.picture.grid[x][y];
                    let color_info = &self.picture.palette[&cell];
                    let solved = self.solved_mask[x][y] || self.report_stale;

                    for shape in cell_shape(color_info, solved, x, y, &to_screen) {
                        shapes.push(shape);
                    }
                }
            }

            // Grid lines:
            for y in 0..=y_size {
                let points = [
                    to_screen * Pos2::new(0.0, y as f32),
                    to_screen * Pos2::new(x_size as f32, y as f32),
                ];
                let stroke = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_black_alpha(if y % 5 == 0 { 64 } else { 16 }),
                );
                shapes.push(egui::Shape::line_segment(points, stroke));
            }
            for x in 0..=x_size {
                let points = [
                    to_screen * Pos2::new(x as f32, 0.0),
                    to_screen * Pos2::new(x as f32, y_size as f32),
                ];
                let stroke = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_black_alpha(if x % 5 == 0 { 64 } else { 16 }),
                );
                shapes.push(egui::Shape::line_segment(points, stroke));
            }

            painter.extend(shapes);
            response.mark_changed();
            response
        });
    }
}

impl eframe::App for NonogramGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _background_color = Color32::from_rgb(
            self.picture.palette[&BACKGROUND].rgb.0,
            self.picture.palette[&BACKGROUND].rgb.1,
            self.picture.palette[&BACKGROUND].rgb.2,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Puzzle Editor");
                if ui.button("+").clicked() || ui.input(|i| i.key_pressed(egui::Key::Equals)) {
                    self.scale = (self.scale + 2.0).min(50.0);
                }
                if ui.button("-").clicked() || ui.input(|i| i.key_pressed(egui::Key::Minus)) {
                    self.scale = (self.scale - 2.0).max(1.0);
                }
            });

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    self.palette_editor(ui);
                    ui.checkbox(&mut self.auto_solve, "auto-solve");
                    if ui.button("Solve").clicked() || (self.auto_solve && self.report_stale) {
                        let puzzle = if self.clue_style == ClueStyle::Triano {
                            Triano::to_dyn(solution_to_triano_puzzle(&self.picture))
                        } else {
                            Nono::to_dyn(solution_to_puzzle(&self.picture))
                        };

                        match puzzle.solve(false) {
                            Ok(grid_solve::Report {
                                skims,
                                scrubs,
                                cells_left,
                                solved_mask,
                            }) => {
                                self.solve_report = format!("{}/{}/{}", skims, scrubs, cells_left);
                                self.solved_mask = solved_mask;
                            }
                            Err(e) => self.solve_report = format!("Error: {:?}", e),
                        }
                        self.report_stale = false;
                    }

                    ui.colored_label(
                        if self.report_stale {
                            Color32::GRAY
                        } else {
                            Color32::BLACK
                        },
                        &self.solve_report,
                    );
                });

                self.canvas(ui);
            });
        });
    }
}

pub fn edit_image(puzzle: &mut Solution, clue_style: ClueStyle) {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Puzzle Editor",
        native_options,
        Box::new(|cc| {
            let style = Style {
                visuals: Visuals::light(),
                ..Style::default()
            };
            cc.egui_ctx.set_style(style);
            Ok(Box::new(NonogramGui::new(cc, puzzle.clone(), clue_style)))
        }),
    )
    .unwrap()
}
