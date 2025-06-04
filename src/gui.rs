use std::path::PathBuf;

use crate::{
    grid_solve,
    puzzle::{Color, ColorInfo, Corner, Solution, BACKGROUND},
};
use egui::{Color32, Frame, Pos2, Rect, RichText, Shape, Style, Vec2, Visuals};
use egui_material_icons::icons;

struct NonogramGui {
    picture: Solution,
    current_color: Color,
    scale: f32,
    filename: PathBuf,

    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,

    auto_solve: bool,
    solve_report: String,
    report_stale: bool,
    solved_mask: Vec<Vec<bool>>,
}

#[derive(Clone, Debug)]
enum Action {
    ChangeColor { changes: Vec<(usize, usize, Color)> },
}

enum ActionMood {
    Creative,
    Undo,
    Redo,
}

impl NonogramGui {
    fn new(cc: &eframe::CreationContext<'_>, picture: Solution) -> Self {
        egui_material_icons::initialize(&cc.egui_ctx);
        let solved_mask = vec![vec![false; picture.grid[0].len()]; picture.grid.len()];

        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        NonogramGui {
            picture,
            current_color: BACKGROUND,
            scale: 10.0,
            filename: PathBuf::new(), // TODO: integrate with everything else better!

            undo_stack: vec![],
            redo_stack: vec![],

            auto_solve: false,
            solve_report: "".to_string(),
            report_stale: true,
            solved_mask,
        }
    }

    fn reversed(&self, action: &Action) -> Action {
        match action {
            Action::ChangeColor { changes } => Action::ChangeColor {
                changes: changes
                    .iter()
                    .map(|(x, y, _)| (*x, *y, self.picture.grid[*x][*y]))
                    .collect::<Vec<_>>(),
            },
        }
    }

    fn perform(&mut self, action: &Action, mood: ActionMood) {
        let reversed_action = self.reversed(&action);

        match action {
            Action::ChangeColor { changes } => {
                for (x, y, old_color) in changes {
                    self.picture.grid[*x][*y] = *old_color;
                }
                self.report_stale = true;
            }
        }

        match mood {
            ActionMood::Creative => {
                self.undo_stack.push(reversed_action);
                self.redo_stack.clear();
            }
            ActionMood::Undo => {
                self.redo_stack.push(reversed_action);
            }
            ActionMood::Redo => {
                self.undo_stack.push(reversed_action);
            }
        }
    }

    fn un_or_re_do(&mut self, un: bool) {
        let action = if un {
            self.undo_stack.pop()
        } else {
            self.redo_stack.pop()
        };

        let action = match action {
            Some(action) => action,
            None => return,
        };

        self.perform(
            &action,
            if un {
                ActionMood::Undo
            } else {
                ActionMood::Redo
            },
        );
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
        let mut removed_color = None;
        let mut add_color = false;

        use itertools::Itertools;

        for (color, color_info) in self
            .picture
            .palette
            .iter_mut()
            .sorted_by_key(|(color, _)| *color)
        {
            let (r, g, b) = color_info.rgb;
            let button_text = if color_info.corner.is_some() {
                color_info.ch.to_string()
            } else {
                "■".to_string()
            };

            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(button_text)
                            .monospace()
                            .size(24.0)
                            .color(egui::Color32::from_rgb(r, g, b)),
                    )
                    .clicked()
                {
                    picked_color = *color;
                };
                let mut edited_color = [r as f32 / 256.0, g as f32 / 256.0, b as f32 / 256.0];

                if ui.color_edit_button_rgb(&mut edited_color).changed() {
                    picked_color = *color;
                    color_info.rgb = (
                        (edited_color[0] * 256.0) as u8,
                        (edited_color[1] * 256.0) as u8,
                        (edited_color[2] * 256.0) as u8,
                    );
                }
                if *color != BACKGROUND {
                    if ui.button(icons::ICON_DELETE).clicked() {
                        removed_color = Some(*color);
                    }
                }
            });
        }
        if ui.button("New color").clicked() {
            add_color = true;
        }
        self.current_color = picked_color;

        if Some(self.current_color) == removed_color {
            self.current_color = BACKGROUND;
        }

        if let Some(removed_color) = removed_color {
            for row in self.picture.grid.iter_mut() {
                for cell in row.iter_mut() {
                    if *cell == removed_color {
                        *cell = self.current_color;
                    }
                }
            }

            self.picture.palette.remove(&removed_color);
        }
        if add_color {
            let next_color = Color(self.picture.palette.keys().map(|k| k.0).max().unwrap() + 1);
            self.picture.palette.insert(
                next_color,
                ColorInfo {
                    ch: (next_color.0 + 65) as char, // TODO: will break chargrid export
                    name: "New color".to_string(),
                    rgb: (128, 128, 128),
                    color: next_color,
                    corner: None,
                },
            );
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let x_size = self.picture.grid.len();
        let y_size = self.picture.grid.first().unwrap().len();

        Frame::canvas(ui.style()).show(ui, |ui| {
            let (mut response, painter) = ui.allocate_painter(
                Vec2::new(self.scale * x_size as f32, self.scale * y_size as f32)
                    + Vec2::new(2.0, 2.0), // for the border
                egui::Sense::click_and_drag(),
            );

            let canvas_without_border = response.rect.shrink(1.0);

            let to_screen = egui::emath::RectTransform::from_to(
                Rect::from_min_size(Pos2::ZERO, Vec2::new(x_size as f32, y_size as f32)),
                canvas_without_border,
            );
            let from_screen = to_screen.inverse();

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if response.clicked() || response.dragged() {
                    let canvas_pos = from_screen * pointer_pos;
                    let x = canvas_pos.x as usize;
                    let y = canvas_pos.y as usize;

                    if (0..x_size).contains(&x) && (0..y_size).contains(&y) {
                        let new_color = if self.picture.grid[x][y] == self.current_color {
                            BACKGROUND
                        } else {
                            self.current_color
                        };
                        self.perform(
                            &Action::ChangeColor {
                                changes: vec![(x, y, new_color)],
                            },
                            ActionMood::Creative,
                        );
                    }
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

    fn loader(&mut self, ui: &mut egui::Ui) {
        if ui.button(icons::ICON_FILE_OPEN).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("image", &["png", "gif", "bmp"])
                .add_filter("PBN", &["xml", "pbn"])
                .add_filter("chargrid", &["txt"])
                .add_filter("Olsak", &["g"])
                .set_directory(".")
                .pick_file()
            {
                let (puzzle, solution) = crate::import::load(&path, None);

                let solution = solution.unwrap_or_else(|| match puzzle.solve(false) {
                    Ok(report) => {
                        self.solved_mask = report.solved_mask;
                        report.solution
                    }
                    Err(_) => panic!("Impossible puzzle!"),
                });
                self.solved_mask = vec![vec![false; solution.grid[0].len()]; solution.grid.len()];

                self.picture = solution;
                self.filename = path;
                self.report_stale = true;
            }
        }
    }

    fn saver(&mut self, ui: &mut egui::Ui) {
        if ui.button(icons::ICON_FILE_SAVE).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("image", &["png", "gif", "bmp"])
                .add_filter("PBN", &["xml", "pbn"])
                .add_filter("chargrid", &["txt"])
                .add_filter("Olsak", &["g"])
                .set_directory(".")
                .save_file()
            {
                crate::export::save(None, Some(&self.picture), &path, None).unwrap();
                self.filename = path;
            }
        }
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
                if ui.button(icons::ICON_ZOOM_IN).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Equals))
                {
                    self.scale = (self.scale + 2.0).min(50.0);
                }
                if ui.button(icons::ICON_ZOOM_OUT).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Minus))
                {
                    self.scale = (self.scale - 2.0).max(1.0);
                }
                self.loader(ui);
                self.saver(ui);
            });

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(100.0);
                    ui.horizontal(|ui| {
                        ui.label(format!("({})", self.undo_stack.len()));
                        if ui.button(icons::ICON_UNDO).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Z))
                        {
                            self.un_or_re_do(true);
                        }
                        if ui.button(icons::ICON_REDO).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Y))
                        {
                            self.un_or_re_do(false);
                        }
                        ui.label(format!("({})", self.redo_stack.len()));
                    });

                    ui.separator();

                    self.palette_editor(ui);

                    ui.separator();
                    ui.checkbox(&mut self.auto_solve, "auto-solve");
                    if ui.button("Solve").clicked() || (self.auto_solve && self.report_stale) {
                        let puzzle = self.picture.to_puzzle();

                        match puzzle.solve(false) {
                            Ok(grid_solve::Report {
                                skims,
                                scrubs,
                                cells_left,
                                solution: _solution,
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

pub fn edit_image(solution: Solution) {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Puzzle Editor",
        native_options,
        Box::new(|cc| {
            let spacing = egui::Spacing {
                interact_size: Vec2::new(20.0, 20.0), // Used by the color-picker buttons
                ..egui::Spacing::default()
            };
            let style = Style {
                visuals: Visuals::light(),
                spacing,

                ..Style::default()
            };
            cc.egui_ctx.set_style(style);
            Ok(Box::new(NonogramGui::new(cc, solution)))
        }),
    )
    .unwrap()
}
