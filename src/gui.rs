use egui::{Color32, Frame, PointerState, Pos2, Rect, RichText, Shape, Style, Vec2, Visuals};

use crate::{
    grid_solve,
    import::{solution_to_puzzle, solution_to_triano_puzzle},
    puzzle::{Clue, Color, ColorInfo, Corner, Nono, Puzzle, Solution, Triano, BACKGROUND},
};

struct MyEguiApp {
    picture: Solution,
    current_color: Color,
    scale: f32,
    auto_solve: bool,
    solve_report: String,
    report_stale: bool,
}

impl MyEguiApp {
    fn new(cc: &eframe::CreationContext<'_>, picture: Solution) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        MyEguiApp {
            picture,
            current_color: BACKGROUND,
            scale: 10.0,
            auto_solve: false,
            solve_report: "".to_string(),
            report_stale: true,
        }
    }
}

fn cell_shape(
    ci: &ColorInfo,
    x: usize,
    y: usize,
    to_screen: &egui::emath::RectTransform,
) -> egui::Shape {
    let (r, g, b) = ci.rgb;
    let color = egui::Color32::from_rgb(r, g, b);

    match ci.corner {
        None => egui::Shape::rect_filled(
            Rect::from_min_size(to_screen * Pos2::new(x as f32, y as f32), to_screen.scale()),
            0.0,
            color,
        ),
        Some(Corner { left, upper }) => {
            let mut points = vec![];
            if left || upper {
                points.push(to_screen * Pos2::new(x as f32, y as f32));
            }
            if !left || upper {
                points.push(to_screen * Pos2::new((x + 1) as f32, y as f32));
            }
            if !left || !upper {
                points.push(to_screen * Pos2::new((x + 1) as f32, (y + 1) as f32));
            }
            if left || !upper {
                points.push(to_screen * Pos2::new(x as f32, (y + 1) as f32));
            }

            Shape::convex_polygon(points, color, (0.0, color))
        }
    }
}

impl eframe::App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _background_color = Color32::from_rgb(
            self.picture.palette[&BACKGROUND].rgb.0,
            self.picture.palette[&BACKGROUND].rgb.1,
            self.picture.palette[&BACKGROUND].rgb.2,
        );

        let x_size = self.picture.grid.len();
        let y_size = self.picture.grid.first().unwrap().len();

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut picked_color = self.current_color;
            let mut new_color_rgb = None;

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
                    for (color, color_info) in &self.picture.palette {
                        let (r, g, b) = &color_info.rgb;

                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    RichText::new(&color_info.ch.to_string())
                                        .monospace()
                                        .color(egui::Color32::from_rgb(*r, *g, *b)),
                                )
                                .clicked()
                            {
                                picked_color = *color;
                            };
                            let mut edited_color =
                                [*r as f32 / 256.0, *g as f32 / 256.0, *b as f32 / 256.0];

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
                    let mut triano = false;
                    ui.checkbox(&mut triano, "Trianogram");
                    ui.checkbox(&mut self.auto_solve, "auto-solve");
                    if ui.button("Solve").clicked() || (self.auto_solve && self.report_stale) {
                        let puzzle = if triano {
                            Triano::to_dyn(solution_to_triano_puzzle(&self.picture))
                        } else {
                            Nono::to_dyn(solution_to_puzzle(&self.picture))
                        };

                        match puzzle.solve(false) {
                            Ok(report) => {
                                self.solve_report = "ok".to_string();
                            }
                            Err(e) => self.solve_report = format!("Error solving puzzle: {}", e),
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
                        let canvas_pos = from_screen * pointer_pos;
                        let x = canvas_pos.x as usize;
                        let y = canvas_pos.y as usize;

                        if (0..x_size).contains(&x) && (0..y_size).contains(&y) {
                            self.picture.grid[x][y] = picked_color;
                        }
                    }

                    let mut shapes = vec![];

                    for y in 0..y_size {
                        for x in 0..x_size {
                            let cell = self.picture.grid[x][y];
                            let color_info = &self.picture.palette[&cell];

                            shapes.push(cell_shape(color_info, x, y, &to_screen));
                        }
                    }
                    painter.extend(shapes);
                    response.mark_changed();
                    response
                });
            });
        });
    }
}

pub fn edit_image(puzzle: &mut Solution) {
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
            Ok(Box::new(MyEguiApp::new(cc, puzzle.clone())))
        }),
    )
    .unwrap();
}
