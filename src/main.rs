use display_info::DisplayInfo;
use egui::Rect;
use enigo::{Button, Enigo, Keyboard, Mouse, Settings};

use eframe::{egui, App};

use eframe::egui::ViewportCommand;
use egui::{pos2, vec2, Color32, Key, Pos2, Rounding, ScrollArea, Stroke, Vec2};
use std::fmt;
use std::mem::zeroed;
use std::str::FromStr;
use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use device_query::{DeviceQuery, DeviceState, Keycode};

// TODO: Read config file for offsets based on ids? Or primary?
// TODO: For primary display, use results from
// $ xprop -root '_NET_WORKAREA'
// _NET_WORKAREA(CARDINAL) = 72, 27, 1848, 1053, 72, 27, 1848, 1053
// First 2 values are the offset

#[derive(Clone, Copy)]
struct Display {
    id: i32,
    pos: Pos2,
    size: Vec2,
    offset: Vec2,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonScreenModeBindings {
    left_region: [String; 4],
    right_region: [String; 4],
    prev_screen: String,
    next_screen: String,

    left_grid: [String; 15],
    right_grid: [String; 15],

    cell_mouse_input: [String; 9],
    cell_mouse_movement: [String; 4],
}

fn to_keycode(s: &str) -> Key {
    let msg = format!("Unable to parse keybinding {}", s);
    if s == "LShift" {
        return Key::F20;
    } else if s == "LControl" {
        return Key::F21;
    }
    return Key::from_name(s).expect(&msg);
}

impl JsonScreenModeBindings {
    fn transform(&self) -> ScreenModeBindings {
        let mut left_region = [Key::Space; 4];
        let mut right_region = [Key::Space; 4];
        for (i, val) in self.left_region.iter().enumerate() {
            left_region[i] = to_keycode(val);
        }
        for (i, val) in self.right_region.iter().enumerate() {
            right_region[i] = to_keycode(val);
        }

        let mut left_grid = [Key::Space; 15];
        for (i, val) in self.left_grid.iter().enumerate() {
            left_grid[i] = to_keycode(val);
        }

        let mut right_grid = [Key::Space; 15];
        for (i, val) in self.right_grid.iter().enumerate() {
            right_grid[i] = to_keycode(val);
        }

        let mut cell_mouse_input = [Key::Space; 9];
        for (i, val) in self.cell_mouse_input.iter().enumerate() {
            cell_mouse_input[i] = to_keycode(val);
        }
        let mut cell_mouse_movement = [Key::Space; 4];
        for (i, val) in self.cell_mouse_movement.iter().enumerate() {
            cell_mouse_movement[i] = to_keycode(val);
        }

        ScreenModeBindings {
            left_region,
            right_region,
            prev_screen: to_keycode(&self.prev_screen),
            next_screen: to_keycode(&self.next_screen),
            left_grid,
            right_grid,
            mouse: MouseBindings {
                left_click_and_exit: cell_mouse_input[0],
                left_click: cell_mouse_input[1],
                middle_click: cell_mouse_input[2],
                right_click: cell_mouse_input[3],
                scroll_up: cell_mouse_input[4],
                scroll_down: cell_mouse_input[5],
                down: cell_mouse_input[6],
                up: cell_mouse_input[7],
                exit: cell_mouse_input[8],

                move_up: cell_mouse_movement[0],
                move_down: cell_mouse_movement[1],
                move_left: cell_mouse_movement[2],
                move_right: cell_mouse_movement[3],
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MouseBindings {
    left_click_and_exit: Key,
    left_click: Key,
    middle_click: Key,
    right_click: Key,
    scroll_up: Key,
    scroll_down: Key,
    down: Key,
    up: Key,
    exit: Key,

    move_up: Key,
    move_down: Key,
    move_left: Key,
    move_right: Key,
}

#[derive(Debug, Clone, Copy)]
struct ScreenModeBindings {
    prev_screen: Key,
    next_screen: Key,

    left_region: [Key; 4],
    right_region: [Key; 4],

    left_grid: [Key; 15],
    right_grid: [Key; 15],

    mouse: MouseBindings,
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct Color(u8, u8, u8, u8);

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct StyleConfig {
    region_line1: Color,
    region_line2: Color,
    left_grid: Color,
    right_grid: Color,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonConfig {
    primary_offset_x: i32,
    primary_offset_y: i32,
    screen_mode_bindings: JsonScreenModeBindings,
    style: StyleConfig,
    scroll_speed: i32,
    movement_speed: i32,
}

impl JsonConfig {
    fn transform(&self) -> Config {
        Config {
            primary_offset_x: self.primary_offset_x,
            primary_offset_y: self.primary_offset_y,
            screen_mode_bindings: self.screen_mode_bindings.transform(),
            style: self.style,
            scroll_speed: self.scroll_speed,
            movement_speed: self.movement_speed,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Config {
    primary_offset_x: i32,
    primary_offset_y: i32,
    screen_mode_bindings: ScreenModeBindings,
    style: StyleConfig,
    scroll_speed: i32,
    movement_speed: i32,
}

#[derive(PartialEq)]
enum Mode {
    Screen,
    Narrow,
    Cell,
}

fn main() -> eframe::Result {
    let res = File::open("config.json");
    let mut res = res.expect("Config file not present!");
    let mut config = String::new();
    res.read_to_string(&mut config)
        .expect("Unable to read config file!");

    let config: JsonConfig = serde_json::from_str(&config).expect("Unable to deserialize config!");
    let config = config.transform();
    println!("Config {config:#?}");

    let display_infos = DisplayInfo::all().expect("Unable to get display info!");
    let displays: Vec<_> = display_infos
        .iter()
        .map(|d| Display {
            id: d.id as i32,
            pos: pos2(d.x as f32, d.y as f32),
            size: vec2(d.width as f32, d.height as f32),
            offset: if d.is_primary {
                vec2(
                    config.primary_offset_x as f32,
                    config.primary_offset_y as f32,
                )
            } else {
                vec2(0.0, 0.0)
            },
        })
        .collect();

    let mouse_pos = DeviceState::new().query_pointer().coords;
    let mouse_pos = pos2(mouse_pos.0 as f32, mouse_pos.1 as f32);
    let mut initial_display_idx = 0;
    for (i, d) in displays.iter().enumerate() {
        if egui::Rect::from_min_size(d.pos, d.size).contains(mouse_pos) {
            initial_display_idx = i;
            break;
        }
    }

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false) // Hide the OS-specific "chrome" around the window
            .with_mouse_passthrough(true)
            .with_always_on_top()
            .with_transparent(true)
            .with_position(displays[initial_display_idx].pos)
            .with_resizable(false)
            .with_maximized(false)
            .with_inner_size(displays[initial_display_idx].size)
            // .with_inner_size(vec2(500.0, 500.0))
            // .with_max_inner_size(vec2(500.0, 500.0))
            // .with_min_inner_size(vec2(500.0, 500.0))
            // .with_inner_size(vec2(w as f32, h as f32))
            // .with_max_inner_size(vec2(w as f32, h as f32))
            // .with_min_inner_size(vec2(w as f32, h as f32))
            .with_fullscreen(false),
        ..Default::default()
    };

    let device_state = DeviceState::new();
    let keys: Vec<Keycode> = device_state.get_keys();
    println!("{keys:#?}");

    let app = MyApp {
        state: SharedState {
            displays,
            current_display: initial_display_idx,
            config,
            mode: Mode::Screen,
            region: 0,
            cell: -1,
        },
    };

    eframe::run_native(
        "Custom window frame", // unused title
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

struct MyApp {
    state: SharedState,
}

struct SharedState {
    displays: Vec<Display>,
    current_display: usize,
    config: Config,
    mode: Mode,
    region: i32,
    cell: i32,
}

impl MyApp {
    fn handle_input(&mut self, ctx: &egui::Context) {
        let state = &mut self.state;
        let bindings = &state.config.screen_mode_bindings;
        let input = ctx.input(|i: &egui::InputState| i.clone());
        let is_pressed = |&k| -> bool {
            if k == Key::F20 {
                return input.modifiers.shift;
            } else if k == Key::F21 {
                return input.modifiers.ctrl;
            }
            input.key_pressed(k)
        };

        if state.mode == Mode::Screen {
            let region_bindings = bindings
                .left_region
                .iter()
                .chain(bindings.right_region.iter())
                .enumerate();
            for (i, key) in region_bindings {
                if is_pressed(key) {
                    println!("Is pressed {key:#?}");
                    state.region = i as i32;
                    state.mode = Mode::Narrow;
                    state.cell = -1;
                    ctx.request_repaint();
                    break;
                }
            }

            if is_pressed(&Key::Backspace) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }

            if is_pressed(&state.config.screen_mode_bindings.prev_screen) {
                let next_display = if state.current_display == 0 {
                    state.displays.len() - 1
                } else {
                    state.current_display - 1
                };
                move_to_display(&ctx, &mut self.state, next_display);
            } else if is_pressed(&state.config.screen_mode_bindings.next_screen) {
                let next_display = state.current_display + 1;
                move_to_display(&ctx, &mut self.state, next_display);
            }
        } else if state.mode == Mode::Narrow {
            let grid_bindings = bindings
                .left_grid
                .iter()
                .chain(bindings.right_grid.iter())
                .enumerate();

            for (i, key) in grid_bindings {
                if is_pressed(key) {
                    if state.cell == i as i32 {
                        state.mode = Mode::Cell;
                        break;
                    } else {
                        state.cell = i as i32;

                        let display = state.displays[state.current_display];
                        let region = state.region;
                        let region_size = vec2(display.size.x * 0.5, display.size.y * 0.25);

                        let mut pos = display.pos;
                        if region >= 4 {
                            pos.x += region_size.x;
                        }
                        pos.y += region_size.y * (region % 4) as f32;
                        let col = i % 5;
                        let row = (i % 15) / 5;

                        let cell_size = vec2(region_size.x * 0.1, region_size.y / 3.0);
                        let half_cell_size = cell_size * 0.5;

                        pos.x += col as f32 * cell_size.x;
                        pos.y += row as f32 * cell_size.y;
                        pos += half_cell_size;

                        if i >= 15 {
                            pos.x += region_size.x * 0.5;
                        }

                        let mut enigo = Enigo::new(&Settings::default()).unwrap();
                        enigo.move_mouse(pos.x as i32, pos.y as i32, enigo::Coordinate::Abs);
                        state.mode = Mode::Cell;
                        break;
                    }
                }
            }

            if is_pressed(&Key::Backspace) {
                state.mode = Mode::Screen;
            }
            if is_pressed(&Key::Enter) && state.cell >= 0 {
                state.mode = Mode::Cell;
            }
        } else if state.mode == Mode::Cell {
            let mut enigo = Enigo::new(&Settings::default()).unwrap();

            let mouse_bindings = &bindings.mouse;
            if is_pressed(&mouse_bindings.left_click_and_exit) {
                println!("Click");

                enigo
                    .button(Button::Left, enigo::Direction::Click)
                    .expect("Unable to perform mouse click!");
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if is_pressed(&mouse_bindings.left_click) {
                println!("Click");

                enigo
                    .button(Button::Left, enigo::Direction::Click)
                    .expect("Unable to perform mouse click!");
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            } else if is_pressed(&mouse_bindings.right_click) {
                println!("Right Click");

                enigo
                    .button(Button::Right, enigo::Direction::Click)
                    .expect("Unable to perform mouse click!");
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else if is_pressed(&mouse_bindings.middle_click) {
                println!("Middle Click");

                enigo
                    .button(Button::Middle, enigo::Direction::Click)
                    .expect("Unable to perform mouse click!");
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else if is_pressed(&mouse_bindings.scroll_up) {
                println!("Scroll up");
                enigo
                    .scroll(-state.config.scroll_speed, enigo::Axis::Vertical)
                    .expect("Unable to scroll up");
            } else if is_pressed(&mouse_bindings.scroll_down) {
                println!("Scroll down");
                enigo
                    .scroll(state.config.scroll_speed, enigo::Axis::Vertical)
                    .expect("Unable to scroll down");
            } else if is_pressed(&mouse_bindings.down) {
                println!("Press down");
                enigo
                    .button(Button::Left, enigo::Direction::Press)
                    .expect("Unable to press");
            } else if is_pressed(&mouse_bindings.up) {
                println!("Press release");

                enigo
                    .button(Button::Left, enigo::Direction::Release)
                    .expect("Unable to release");
            }
            if is_pressed(&mouse_bindings.exit) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }

            if is_pressed(&mouse_bindings.move_down) {
                enigo.move_mouse(0, state.config.movement_speed, enigo::Coordinate::Rel);
            }
            if is_pressed(&mouse_bindings.move_up) {
                enigo.move_mouse(0, -state.config.movement_speed, enigo::Coordinate::Rel);
            }
            if is_pressed(&mouse_bindings.move_left) {
                enigo.move_mouse(-state.config.movement_speed, 0, enigo::Coordinate::Rel);
            }
            if is_pressed(&mouse_bindings.move_right) {
                enigo.move_mouse(state.config.movement_speed, 0, enigo::Coordinate::Rel);
            }

            if is_pressed(&Key::Backspace) {
                state.mode = Mode::Narrow;
            }
        }
    }
}

impl eframe::App for MyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        self.handle_input(ctx);

        // let current_display = self.current_display.load(Ordering::Acquire);
        // let ref display = self.displays[current_display];
        // let pos = pos2(display.x as f32, display.y as f32);
        // let size = vec2(display.width as f32, display.height as f32);
        // ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        // ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));

        custom_window_frame(ctx, "egui with custom frame", |ui| {
            // ui.label("This is just the contents of the window.");
            // ui.horizontal(|ui| {
            //     ui.label("egui theme:");
            //     egui::widgets::global_theme_preference_buttons(ui);
            // });

            let painter = ui.painter();
            let region = self.state.region;
            let ref display = self.state.displays[self.state.current_display];
            let origin = Pos2::ZERO - display.offset;
            let style = &self.state.config.style;

            let region_line1_color = Color32::from_rgba_unmultiplied(
                style.region_line1.0,
                style.region_line1.1,
                style.region_line1.2,
                style.region_line1.3,
            );
            let region_line2_color = Color32::from_rgba_unmultiplied(
                style.region_line1.0,
                style.region_line1.1,
                style.region_line1.2,
                style.region_line1.3,
            );
            let region_line1_stroke = Stroke::new(5.0, region_line1_color);
            let region_line2_stroke = Stroke::new(3.0, region_line2_color);

            if self.state.mode == Mode::Screen {
                let rect = Rect::from_min_size(origin, display.size).shrink(5.0);
                painter.rect_stroke(rect, Rounding::ZERO, region_line1_stroke);
                painter.rect_stroke(rect, Rounding::ZERO, region_line2_stroke);
                let edges = vec![
                    (
                        origin + vec2(display.size.x * 0.5, 0.0),
                        origin + vec2(display.size.x * 0.5, display.size.y),
                    ),
                    (
                        origin + vec2(0.0, display.size.y * 0.5),
                        origin + vec2(display.size.x, display.size.y * 0.5),
                    ),
                    (
                        origin + vec2(0.0, display.size.y * 0.25),
                        origin + vec2(display.size.x, display.size.y * 0.25),
                    ),
                    (
                        origin + vec2(0.0, display.size.y * 0.75),
                        origin + vec2(display.size.x, display.size.y * 0.75),
                    ),
                ];

                for edge in edges {
                    painter.line_segment([edge.0, edge.1], region_line1_stroke);
                    painter.line_segment([edge.0, edge.1], region_line2_stroke);
                }
            } else if self.state.mode == Mode::Narrow {
                let mut origin = origin;
                if region < 4 {
                    origin += vec2(display.size.x * 0.0, display.size.y * 0.25 * region as f32);
                } else {
                    origin += vec2(
                        display.size.x * 0.5,
                        display.size.y * 0.25 * (region - 4) as f32,
                    );
                }

                let region_size = vec2(display.size.x * 0.5, display.size.y * 0.25);
                for i in 0..11 {
                    let i = i as f32;
                    let start = origin + vec2(region_size.x * i * 0.1, 0.0);
                    let end = origin + vec2(region_size.x * i * 0.1, region_size.y);
                    painter.line_segment([start, end], region_line1_stroke);
                    painter.line_segment([start, end], region_line2_stroke);
                }

                for i in 0..4 {
                    let i = i as f32;
                    let start = origin + vec2(0.0, region_size.y * i * 0.333);
                    let end = origin + vec2(region_size.x, region_size.y * i * 0.333);
                    painter.line_segment([start, end], region_line1_stroke);
                    painter.line_segment([start, end], region_line2_stroke);
                }

                let left_color = Color32::from_rgba_unmultiplied(
                    style.left_grid.0,
                    style.left_grid.1,
                    style.left_grid.2,
                    style.left_grid.3,
                );
                let left_rect = egui::Rect::from_two_pos(
                    origin,
                    origin + vec2(region_size.x * 0.5, region_size.y),
                );
                painter.rect(left_rect, Rounding::ZERO, left_color, Stroke::NONE);

                let right_color = Color32::from_rgba_unmultiplied(
                    style.right_grid.0,
                    style.right_grid.1,
                    style.right_grid.2,
                    style.right_grid.3,
                );
                let right_rect = egui::Rect::from_two_pos(
                    origin + vec2(region_size.x * 0.5, 0.0),
                    origin + vec2(region_size.x, region_size.y),
                );
                painter.rect(right_rect, Rounding::ZERO, right_color, Stroke::NONE);
            } else if self.state.mode == Mode::Cell {
                let mut origin = origin;
                let region_size = vec2(display.size.x * 0.5, display.size.y * 0.25);
                let cell_size = vec2(region_size.x * 0.1, region_size.y / 3.0);

                if region < 4 {
                    origin += vec2(0.0, region_size.y * region as f32);
                } else {
                    origin += vec2(region_size.x, region_size.y * (region - 4) as f32);
                }

                let col = self.state.cell % 5;
                let row = (self.state.cell % 15) / 5;

                origin.x += col as f32 * cell_size.x;
                origin.y += row as f32 * cell_size.y;

                let color = if self.state.cell >= 15 {
                    origin.x += region_size.x * 0.5;
                    style.right_grid
                } else {
                    style.left_grid
                };
                let color = Color32::from_rgba_unmultiplied(color.0, color.1, color.2, color.3);

                let rect = egui::Rect::from_min_size(origin, cell_size);
                painter.rect(rect, Rounding::ZERO, color, Stroke::NONE);
            }

            let color = Color32::from_rgba_premultiplied(28, 92, 48, 120);
            let rect = egui::Rect::from_two_pos(pos2(0.0, 0.0), pos2(50.0, 50.0));
            painter.rect(rect, Rounding::ZERO, color, Stroke::new(0.0, color));

            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.request_repaint();
        });
    }
}

fn move_to_display(ctx: &egui::Context, state: &mut SharedState, display_idx: usize) {
    state.current_display = display_idx % state.displays.len();

    let ref display = state.displays[state.current_display];
    let pos = display.pos + display.offset;
    let size = display.size - display.offset;

    ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
    ctx.request_repaint();
}

fn custom_window_frame(ctx: &egui::Context, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    use egui::{CentralPanel, UiBuilder};
    let panel_frame = egui::Frame::none();

    CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
        let app_rect = ui.max_rect();
        let mut content_ui = ui.new_child(UiBuilder::new().max_rect(app_rect));
        add_contents(&mut content_ui);
    });
}
