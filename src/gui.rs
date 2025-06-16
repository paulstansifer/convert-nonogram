use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        mpsc, Arc, Mutex,
    },
    thread,
};

use crate::{
    export::to_bytes,
    grid_solve::{self, disambig_candidates},
    puzzle::{Color, ColorInfo, Corner, Solution, BACKGROUND},
};
use egui::{Color32, Frame, Pos2, Rect, RichText, Shape, Style, Vec2, Visuals};
use egui_material_icons::icons;

#[cfg(not(target_arch = "wasm32"))]
pub fn edit_image(solution: Solution) {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Number Loom",
        native_options,
        Box::new(|cc| Ok(Box::new(NonogramGui::new(cc, solution)))),
    )
    .unwrap()
}

#[cfg(target_arch = "wasm32")]
pub fn edit_image(solution: Solution) {
    use eframe::wasm_bindgen::JsCast as _;

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let _start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(NonogramGui::new(cc, solution)))),
            )
            .await;

        // The example code removes the spinner here, but it doesn't seem necessary.
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local as spawn_async;

#[cfg(not(target_arch = "wasm32"))]
fn spawn_async<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + std::marker::Send,
{
    // This sort of weird construct allows us to avoid multithreaded tokio,
    // which isn't available on wasm32 (cargo doesn't like having the same crate have different
    // features on different platforms, and we might want to use some tokio features on wasm32)
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(future);
    });
}

struct NonogramGui {
    picture: Solution,
    current_color: Color,
    scale: f32,
    opened_file_receiver: mpsc::Receiver<Solution>,

    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,

    auto_solve: bool,
    lines_to_affect_string: String,

    solve_report: String,
    report_stale: bool,
    disambiguator: Arc<Disambiguator>,

    solved_mask: Vec<Vec<bool>>,
}

#[derive(Clone, Debug)]
enum Action {
    ChangeColor {
        changes: HashMap<(usize, usize), Color>,
    },
    ReplacePicture {
        picture: Solution,
    },
}

#[derive(PartialEq, Eq)]
enum ActionMood {
    Normal,
    Merge,
    Undo,
    Redo,
}

impl NonogramGui {
    fn new(cc: &eframe::CreationContext<'_>, picture: Solution) -> Self {
        egui_material_icons::initialize(&cc.egui_ctx);
        let solved_mask = vec![vec![false; picture.grid[0].len()]; picture.grid.len()];

        NonogramGui {
            picture,
            current_color: BACKGROUND,
            scale: 10.0,
            opened_file_receiver: mpsc::channel().1,

            undo_stack: vec![],
            redo_stack: vec![],

            auto_solve: false,
            lines_to_affect_string: "5".to_string(),

            solve_report: "".to_string(),
            report_stale: true,
            disambiguator: Arc::new(Disambiguator::new()),

            solved_mask,
        }
    }

    fn reversed(&self, action: &Action) -> Action {
        match action {
            Action::ChangeColor { changes } => Action::ChangeColor {
                changes: changes
                    .keys()
                    .map(|(x, y)| ((*x, *y), self.picture.grid[*x][*y]))
                    .collect::<HashMap<_, _>>(),
            },
            Action::ReplacePicture { picture: _ } => Action::ReplacePicture {
                picture: self.picture.clone(),
            },
        }
    }

    fn perform(&mut self, action: Action, mood: ActionMood) {
        use Action::*;
        use ActionMood::*;

        let mood = if mood == Merge {
            match (self.undo_stack.last_mut(), &action) {
                // Consecutive `ChangeColor`s can be merged with each other.
                (
                    Some(ChangeColor { changes }),
                    ChangeColor {
                        changes: new_changes,
                    },
                ) => {
                    for ((x, y), col) in new_changes {
                        if !changes.contains_key(&(*x, *y)) {
                            changes.insert((*x, *y), self.picture.grid[*x][*y]);
                            // Crucially, this only fires on a new cell!
                            // Otherwise, we'd be flipping cells back and forth as long as we were
                            // in them!
                            self.picture.grid[*x][*y] = *col;
                            self.report_stale = true;
                        }
                    }
                    return; // Action is done; nothing else to do!
                }
                _ => Normal, // Unable to merge; add a new undo entry.
            }
        } else {
            mood
        };

        let reversed_action = self.reversed(&action);

        match action {
            Action::ChangeColor { changes } => {
                for ((x, y), new_color) in changes {
                    self.picture.grid[x][y] = new_color;
                }
                self.report_stale = true;
            }
            Action::ReplacePicture { picture } => {
                let solved_mask = vec![vec![false; picture.grid[0].len()]; picture.grid.len()];
                self.picture = picture;
                self.solved_mask = solved_mask;

                self.report_stale = true;
                self.disambiguator.reset();
            }
        }

        match mood {
            Merge => {}
            Normal => {
                self.undo_stack.push(reversed_action);
                self.redo_stack.clear();
            }
            Undo => {
                self.redo_stack.push(reversed_action);
            }
            Redo => {
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
            action,
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
    disambig: (&ColorInfo, f32),
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

    if disambig.1 < 1.0 {
        let (r, g, b) = disambig.0.rgb;
        res.push(egui::Shape::rect_filled(
            Rect::from_min_size(
                to_screen * Pos2::new(x as f32 + 0.25, y as f32 + 0.25),
                to_screen.scale() * 0.5,
            ),
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, ((1.0 - disambig.1) * 255.0) as u8),
        ));
    }

    res
}

impl NonogramGui {
    fn resize(&mut self, top: Option<bool>, left: Option<bool>, add: bool) {
        let mut g = self.picture.grid.clone();
        let lines = match self.lines_to_affect_string.parse::<usize>() {
            Ok(lines) => lines,
            Err(_) => {
                self.lines_to_affect_string += "??";
                return;
            }
        };
        if let Some(left) = left {
            if add {
                g.resize(g.len() + lines, vec![BACKGROUND; g.first().unwrap().len()]);
                if left {
                    g.rotate_right(lines);
                }
            } else {
                if left {
                    g.rotate_left(lines);
                }
                g.truncate(g.len() - lines);
            }
        } else if let Some(top) = top {
            if add {
                for row in g.iter_mut() {
                    row.resize(row.len() + lines, BACKGROUND);
                    if top {
                        row.rotate_right(lines);
                    }
                }
            } else {
                for row in g.iter_mut() {
                    if top {
                        row.rotate_left(lines);
                    }
                    row.truncate(row.len() - lines);
                }
            }
        }

        self.perform(
            Action::ReplacePicture {
                picture: Solution {
                    grid: g,
                    ..self.picture.clone()
                },
            },
            ActionMood::Normal,
        );
    }

    fn resizer(&mut self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Canvas size: {}x{}",
            self.picture.x_size(),
            self.picture.y_size(),
        ));

        egui::Grid::new("resizer").show(ui, |ui| {
            ui.label("");
            ui.horizontal(|ui| {
                if ui.button(icons::ICON_ADD).clicked() {
                    self.resize(Some(true), None, true);
                }
                if ui.button(icons::ICON_REMOVE).clicked() {
                    self.resize(Some(true), None, false);
                }
            });
            ui.label("");
            ui.end_row();

            ui.vertical(|ui| {
                if ui.button(icons::ICON_ADD).clicked() {
                    self.resize(None, Some(true), true);
                }
                if ui.button(icons::ICON_REMOVE).clicked() {
                    self.resize(None, Some(true), false);
                }
            });
            ui.text_edit_singleline(&mut self.lines_to_affect_string);

            ui.vertical(|ui| {
                if ui.button(icons::ICON_ADD).clicked() {
                    self.resize(None, Some(false), true);
                }
                if ui.button(icons::ICON_REMOVE).clicked() {
                    self.resize(None, Some(false), false);
                }
            });
            ui.end_row();

            ui.label("");
            ui.horizontal(|ui| {
                if ui.button(icons::ICON_ADD).clicked() {
                    self.resize(Some(false), None, true);
                }
                if ui.button(icons::ICON_REMOVE).clicked() {
                    self.resize(Some(false), None, false);
                }
            });
            ui.label("");
        });
    }

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
                ui.label(RichText::new(icons::ICON_CHEVRON_FORWARD).size(24.0).color(
                    Color32::from_black_alpha(if *color == picked_color { 255 } else { 0 }),
                ));

                let color_text = RichText::new(button_text)
                    .monospace()
                    .size(24.0)
                    .color(egui::Color32::from_rgb(r, g, b));
                if ui
                    .add_enabled(*color != picked_color, egui::Button::new(color_text))
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
                        let mut changes = HashMap::new();
                        changes.insert((x, y), new_color);
                        self.perform(
                            Action::ChangeColor { changes },
                            if response.clicked() || response.drag_started() {
                                ActionMood::Normal
                            } else {
                                ActionMood::Merge
                            },
                        );
                    }
                }
            }

            let mut shapes = vec![];
            let disambig_report = self.disambiguator.report.lock().unwrap();

            for y in 0..y_size {
                for x in 0..x_size {
                    let cell = self.picture.grid[x][y];
                    let color_info = &self.picture.palette[&cell];
                    let solved = self.solved_mask[x][y] || self.report_stale;

                    let dr = if let Some(disambig_report) = disambig_report.as_ref() {
                        let (c, score) = disambig_report[x][y];
                        (&self.picture.palette[&c], score)
                    } else {
                        (&self.picture.palette[&BACKGROUND], 1.0)
                    };

                    for shape in cell_shape(color_info, solved, dr, x, y, &to_screen) {
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
        if ui.button("New blank").clicked() {
            self.perform(
                Action::ReplacePicture {
                    picture: Solution::blank_bw(20, 20),
                },
                ActionMood::Normal,
            );
        }
        if ui.button("Open").clicked() {
            let (sender, receiver) = mpsc::channel();
            self.opened_file_receiver = receiver;

            spawn_async(async move {
                let handle = rfd::AsyncFileDialog::new()
                    .add_filter(
                        "all recognized formats",
                        &["png", "gif", "bmp", "xml", "pbn", "txt", "g"],
                    )
                    .add_filter("image", &["png", "gif", "bmp"])
                    .add_filter("PBN", &["xml", "pbn"])
                    .add_filter("chargrid", &["txt"])
                    .add_filter("Olsak", &["g"])
                    .pick_file()
                    .await;

                if let Some(handle) = handle {
                    let (puzzle, solution) =
                        crate::import::load(&handle.file_name(), handle.read().await, None);

                    // TODO: at least error out for unsolveable inputs
                    let solution =
                        solution.unwrap_or_else(|| puzzle.plain_solve().unwrap().solution);

                    sender.send(solution).unwrap();
                }
            });
        }

        if let Ok(solution) = self.opened_file_receiver.try_recv() {
            self.perform(
                Action::ReplacePicture { picture: solution },
                ActionMood::Normal,
            );
        }
    }

    fn saver(&mut self, ui: &mut egui::Ui) {
        if ui.button("Save").clicked() {
            let solution_copy = self.picture.clone();
            spawn_async(async move {
                let handle = rfd::AsyncFileDialog::new()
                    .add_filter(
                        "all recognized formats",
                        &["png", "gif", "bmp", "xml", "pbn", "txt", "g", "html"],
                    )
                    .add_filter("image", &["png", "gif", "bmp"])
                    .add_filter("PBN", &["xml", "pbn"])
                    .add_filter("chargrid", &["txt"])
                    .add_filter("Olsak", &["g"])
                    .add_filter("HTML (for printing)", &["html"])
                    .save_file()
                    .await;

                if let Some(handle) = handle {
                    let bytes =
                        to_bytes(None, Some(&solution_copy), Some(handle.file_name()), None)
                            .unwrap();
                    handle.write(&bytes).await.unwrap();
                }
            });
        }
    }
}

impl eframe::App for NonogramGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Styling. Has to be here instead of `edit_image` to take effect on the Web.
        let spacing = egui::Spacing {
            interact_size: Vec2::new(20.0, 20.0), // Used by the color-picker buttons
            ..egui::Spacing::default()
        };
        let style = Style {
            visuals: Visuals::light(),
            spacing,

            ..Style::default()
        };
        ctx.set_style(style);

        let _background_color = Color32::from_rgb(
            self.picture.palette[&BACKGROUND].rgb.0,
            self.picture.palette[&BACKGROUND].rgb.1,
            self.picture.palette[&BACKGROUND].rgb.2,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
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
            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(120.0);
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

                    self.resizer(ui);

                    ui.separator();

                    self.palette_editor(ui);

                    ui.separator();
                    ui.checkbox(&mut self.auto_solve, "auto-solve");
                    if ui.button("Solve").clicked() || (self.auto_solve && self.report_stale) {
                        let puzzle = self.picture.to_puzzle();

                        match puzzle.plain_solve() {
                            Ok(grid_solve::Report {
                                skims,
                                scrubs,
                                cells_left,
                                solution: _solution,
                                solved_mask,
                            }) => {
                                self.solve_report = format!(
                                    "skims: {} scrubs: {} unsolved cells: {}",
                                    skims, scrubs, cells_left
                                );
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

                    ui.separator();

                    Disambiguator::disambig_widget(self.disambiguator.clone(), self, ui);
                });

                self.canvas(ui);
            });
        });
    }
}

struct Disambiguator {
    report: Mutex<Option<Vec<Vec<(Color, f32)>>>>,
    progress: std::sync::atomic::AtomicUsize,
    running: std::sync::atomic::AtomicBool,
    should_stop: std::sync::atomic::AtomicBool,
}

impl Disambiguator {
    fn new() -> Self {
        Disambiguator {
            report: Mutex::new(None),
            progress: AtomicUsize::new(0),
            running: AtomicBool::new(false),
            should_stop: AtomicBool::new(false),
        }
    }

    // Must do this any time the resolution changes!
    // (Currently that only happens through `ReplacePicture`)
    fn reset(&self) {
        *self.report.lock().unwrap() = None;
        self.progress.store(0, std::sync::atomic::Ordering::Relaxed);
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.should_stop
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn disambig_widget(disambiguator: Arc<Self>, overall_gui: &mut NonogramGui, ui: &mut egui::Ui) {
        let progress = disambiguator
            .progress
            .load(std::sync::atomic::Ordering::Relaxed);
        let report_running = progress > 0 && progress < 9_990;

        if !report_running {
            if ui.button("Disambiguate!").clicked() {
                overall_gui.report_stale = true; // Just to clear the dots from the screen.

                let t_d = disambiguator.clone();
                let solution = overall_gui.picture.clone();
                thread::spawn(move || {
                    match disambig_candidates(&solution, &t_d.progress, &t_d.should_stop) {
                        Ok(rep) => {
                            *t_d.report.lock().unwrap() = Some(rep);
                        }
                        Err(_) => {
                            t_d.reset();
                        }
                    }
                });
            }
        } else {
            if ui.button("Stop").clicked() {
                // HACK: we're using `progress`` as a 2-way channel.
                disambiguator
                    .should_stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        ui.add(egui::ProgressBar::new(progress as f32 / 10_000.0).animate(report_running));
        if ui
            .add_enabled(
                !disambiguator.report.lock().unwrap().is_none(),
                egui::Button::new("Clear"),
            )
            .clicked()
        {
            *disambiguator.report.lock().unwrap() = None;
            disambiguator
                .progress
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
