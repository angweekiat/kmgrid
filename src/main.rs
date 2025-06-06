use display_info::DisplayInfo;
use egui::{Align2, Rect};
use enigo::{Button, Enigo, Mouse, Settings};

use eframe::{egui, Result};

use eframe::egui::ViewportCommand;
use egui::{pos2, vec2, Color32, Key, Pos2, Rounding, Stroke, Vec2};
use std::{fs::File, io::Read};

use device_query::{DeviceQuery, DeviceState, Keycode};

#[derive(Clone, Copy)]
struct Display {
    pos: Pos2,
    size: Vec2,
    offset: Vec2,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonBindingsForMouse {
    move_up: String,
    move_down: String,
    move_left: String,
    move_right: String,

    left_click: String,
    left_click_and_exit: String,
    middle_click: String,
    right_click: String,

    left_click_down: String,
    left_click_up: String,

    scroll_up: String,
    scroll_down: String,
    scroll_left: String,
    scroll_right: String,

    speed_quarter: String,
    speed_half: String,
    speed_twice: String,
    speed_quadruple: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonKeyBindings {
    region: [String; 16],
    skip_to_cell: String,
    prev_screen: String,
    next_screen: String,

    grid: [String; 15],

    mouse: JsonBindingsForMouse,
}

fn to_keycode(s: &str) -> Key {
    let msg = format!("Unable to parse keybinding {}", s);
    return Key::from_name(s).expect(&msg);
}

impl JsonKeyBindings {
    fn transform(&self) -> KeyBindings {
        let mut region = [Key::Space; 16];
        for (i, val) in self.region.iter().enumerate() {
            region[i] = to_keycode(val);
        }

        let mut grid = [Key::Space; 15];
        for (i, val) in self.grid.iter().enumerate() {
            grid[i] = to_keycode(val);
        }

        KeyBindings {
            region,
            prev_screen: to_keycode(&self.prev_screen),
            next_screen: to_keycode(&self.next_screen),
            skip_to_cell: to_keycode(&self.skip_to_cell),
            grid,
            mouse: MouseBindings {
                move_up: to_keycode(&self.mouse.move_up),
                move_down: to_keycode(&self.mouse.move_down),
                move_left: to_keycode(&self.mouse.move_left),
                move_right: to_keycode(&self.mouse.move_right),

                left_click: to_keycode(&self.mouse.left_click),
                left_click_and_exit: to_keycode(&self.mouse.left_click_and_exit),
                middle_click: to_keycode(&self.mouse.middle_click),
                right_click: to_keycode(&self.mouse.right_click),

                left_click_down: to_keycode(&self.mouse.left_click_down),
                left_click_up: to_keycode(&self.mouse.left_click_up),

                scroll_up: to_keycode(&self.mouse.scroll_up),
                scroll_down: to_keycode(&self.mouse.scroll_down),
                scroll_left: to_keycode(&self.mouse.scroll_left),
                scroll_right: to_keycode(&self.mouse.scroll_right),

                speed_quarter: to_keycode(&self.mouse.speed_quarter),
                speed_half: to_keycode(&self.mouse.speed_half),
                speed_twice: to_keycode(&self.mouse.speed_twice),
                speed_quadruple: to_keycode(&self.mouse.speed_quadruple),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MouseBindings {
    move_up: Key,
    move_down: Key,
    move_left: Key,
    move_right: Key,

    left_click: Key,
    left_click_and_exit: Key,
    middle_click: Key,
    right_click: Key,

    left_click_down: Key,
    left_click_up: Key,

    scroll_up: Key,
    scroll_down: Key,
    scroll_left: Key,
    scroll_right: Key,

    speed_quarter: Key,
    speed_half: Key,
    speed_twice: Key,
    speed_quadruple: Key,
}

#[derive(Debug, Clone, Copy)]
struct KeyBindings {
    prev_screen: Key,
    next_screen: Key,

    region: [Key; 16],
    skip_to_cell: Key,

    grid: [Key; 15],

    mouse: MouseBindings,
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct Color(u8, u8, u8, u8);

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct StyleConfig {
    region_line1: Color,
    region_line2: Color,
    region_grid_line1: Color,
    region_grid_line2: Color,
    left_grid: Color,
    right_grid: Color,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonConfig {
    primary_offset_x: i32,
    primary_offset_y: i32,
    key_bindings: JsonKeyBindings,
    style: StyleConfig,
    scroll_speed: i32,
    movement_speed: i32,
}

impl JsonConfig {
    fn transform(&self) -> Config {
        Config {
            primary_offset_x: self.primary_offset_x,
            primary_offset_y: self.primary_offset_y,
            key_bindings: self.key_bindings.transform(),
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
    key_bindings: KeyBindings,
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
    let mut config = String::new();
    let res: Result<File, std::io::Error> = File::open("config.json");
    if let Ok(file) = res {
        let mut res = file;
        res.read_to_string(&mut config)
            .expect("Unable to read config file!");
    } else {
        let args: Vec<String> = std::env::args().collect();
        assert!(args.len() == 2);
        let res: Result<File, std::io::Error> = File::open(&args[1]);
        res.expect("Unable to find config file!")
            .read_to_string(&mut config)
            .expect("Unable to read config file!");
    }

    let config: JsonConfig = serde_json::from_str(&config).expect("Unable to deserialize config!");
    let config = config.transform();
    println!("Config {config:#?}");

    let display_infos = DisplayInfo::all().expect("Unable to get display info!");
    let displays: Vec<_> = display_infos
        .iter()
        .map(|d| Display {
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
            .with_window_type(egui::X11WindowType::Utility)
            .with_mouse_passthrough(true)
            .with_always_on_top()
            .with_transparent(true)
            .with_position(displays[initial_display_idx].pos)
            .with_resizable(false)
            .with_maximized(false)
            .with_inner_size(displays[initial_display_idx].size)
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
            device_state: device_query::DeviceState::new(),
            enigo: Enigo::new(&Settings::default()).unwrap(),
            mouse_key_down: std::collections::HashSet::new(),
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
    device_state: DeviceState,
    enigo: Enigo,
    mouse_key_down: std::collections::HashSet<Key>,
}

impl MyApp {
    fn move_to_display(&mut self, ctx: &egui::Context, display_idx: usize) {
        self.state.current_display = display_idx % self.state.displays.len();

        let ref display = self.state.displays[self.state.current_display];
        let pos = display.pos + display.offset;
        let size = display.size - display.offset;

        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        ctx.request_repaint();
    }

    fn handle_screen_input<F>(&mut self, ctx: &egui::Context, is_pressed: F)
    where
        F: Fn(Key) -> bool,
    {
        let region_bindings = self.state.config.key_bindings.region.iter().enumerate();
        for (i, key) in region_bindings {
            if is_pressed(*key) {
                self.state.region = i as i32;
                self.state.mode = Mode::Narrow;
                self.state.cell = -1;
                ctx.request_repaint();
                break;
            }
        }

        if is_pressed(Key::Backspace) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if is_pressed(self.state.config.key_bindings.skip_to_cell) {
            self.skip_to_cell(ctx);
        }
        if is_pressed(self.state.config.key_bindings.prev_screen) {
            let next_display = if self.state.current_display == 0 {
                self.state.displays.len() - 1
            } else {
                self.state.current_display - 1
            };
            self.move_to_display(&ctx, next_display);
        } else if is_pressed(self.state.config.key_bindings.next_screen) {
            let next_display = self.state.current_display + 1;
            self.move_to_display(&ctx, next_display);
        }
    }

    fn handle_grid_input<F>(&mut self, is_pressed: F) -> Result<(), enigo::InputError>
    where
        F: Fn(Key) -> bool,
    {
        let bindings: &KeyBindings = &self.state.config.key_bindings;
        let grid_bindings = bindings.grid.iter().enumerate();

        for (i, key) in grid_bindings {
            if is_pressed(*key) {
                self.state.cell = i as i32;

                let display = self.state.displays[self.state.current_display];
                let region = self.state.region;
                let region_size = vec2(display.size.x * 0.25, display.size.y * 0.25);
                let cell_size = vec2(region_size.x / 5.0, region_size.y / 3.0);

                let mut pos = display.pos;
                pos += vec2(
                    region_size.x * (region % 4) as f32,
                    region_size.y * (region / 4) as f32,
                ) + vec2(
                    cell_size.x * ((i % 5) as f32 + 0.5),
                    cell_size.y * ((i / 5) as f32 + 0.5),
                );

                self.state
                    .enigo
                    .move_mouse(pos.x as i32, pos.y as i32, enigo::Coordinate::Abs)?;
                self.state.mode = Mode::Cell;

                self.state.mouse_key_down.clear();
                break;
            }
        }

        if is_pressed(Key::Backspace) {
            self.state.mode = Mode::Screen;
        }
        if is_pressed(Key::Enter) && self.state.cell >= 0 {
            self.state.mode = Mode::Cell;
        }
        return Ok(());
    }

    fn handle_cell_input<F1, F2>(
        &mut self,
        ctx: &egui::Context,
        is_pressed: F1,
        is_held: F2,
    ) -> Result<(), enigo::InputError>
    where
        F1: Fn(Key) -> bool,
        F2: Fn(Key) -> bool,
    {
        let mut is_held_with_check = |k| -> bool {
            if self.state.mouse_key_down.contains(&k) {
                return is_held(k);
            } else if !is_held(k) {
                if !self.state.mouse_key_down.contains(&k) {
                    self.state.mouse_key_down.insert(k);
                }
            }
            false
        };

        let bindings = &self.state.config.key_bindings.mouse;
        let enigo = &mut self.state.enigo;

        if is_pressed(bindings.left_click_and_exit) {
            println!("Click and bye!");

            enigo.button(Button::Left, enigo::Direction::Click)?;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if is_pressed(bindings.left_click) {
            println!("Click");

            enigo.button(Button::Left, enigo::Direction::Click)?;
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        } else if is_pressed(bindings.right_click) {
            println!("Right Click");

            enigo.button(Button::Right, enigo::Direction::Click)?;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if is_pressed(bindings.middle_click) {
            println!("Middle Click");

            enigo.button(Button::Middle, enigo::Direction::Click)?;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        if is_held_with_check(bindings.scroll_up) {
            println!("Scroll up");
            enigo.scroll(-self.state.config.scroll_speed, enigo::Axis::Vertical)?;

            enigo.move_mouse(0, 0, enigo::Coordinate::Rel)?;
        } else if is_held_with_check(bindings.scroll_down) {
            println!("Scroll down");
            enigo.scroll(self.state.config.scroll_speed, enigo::Axis::Vertical)?;

            enigo.move_mouse(0, 0, enigo::Coordinate::Rel)?;
        } else if is_held_with_check(bindings.scroll_left) {
            println!("Scroll left");
            enigo.scroll(-self.state.config.scroll_speed, enigo::Axis::Horizontal)?;

            enigo.move_mouse(0, 0, enigo::Coordinate::Rel)?;
        } else if is_held_with_check(bindings.scroll_right) {
            println!("Scroll right");
            enigo.scroll(self.state.config.scroll_speed, enigo::Axis::Horizontal)?;

            enigo.move_mouse(0, 0, enigo::Coordinate::Rel)?;
        }

        if is_pressed(bindings.left_click_down) {
            println!("Press down");
            enigo.button(Button::Left, enigo::Direction::Press)?;
        } else if is_pressed(bindings.left_click_up) {
            println!("Press release");

            enigo.button(Button::Left, enigo::Direction::Release)?;
        }

        let mut dist = self.state.config.movement_speed;
        if is_held(bindings.speed_quarter) {
            dist /= 4;
        }
        if is_held(bindings.speed_half) {
            dist /= 2;
        }
        if is_held(bindings.speed_twice) {
            dist *= 2;
        }
        if is_held(bindings.speed_quadruple) {
            dist *= 4;
        }

        if is_held_with_check(bindings.move_down) {
            enigo.move_mouse(0, dist, enigo::Coordinate::Rel)?;
        }
        if is_held_with_check(bindings.move_up) {
            enigo.move_mouse(0, -dist, enigo::Coordinate::Rel)?;
        }
        if is_held_with_check(bindings.move_left) {
            enigo.move_mouse(-dist, 0, enigo::Coordinate::Rel)?;
        }
        if is_held_with_check(bindings.move_right) {
            enigo.move_mouse(dist, 0, enigo::Coordinate::Rel)?;
        }

        if is_pressed(Key::Backspace) {
            self.state.mode = Mode::Narrow;
        }
        return Ok(());
    }

    fn handle_input(&mut self, ctx: &egui::Context) -> Result<(), enigo::InputError> {
        let input = ctx.input(|i: &egui::InputState| i.clone());

        let is_pressed = |k| -> bool { input.key_pressed(k) };
        let is_held = |k| -> bool { input.key_down(k) };

        if is_pressed(Key::Escape) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if self.state.mode == Mode::Screen {
            self.handle_screen_input(ctx, &is_pressed);
        } else if self.state.mode == Mode::Narrow {
            self.handle_grid_input(&is_pressed)?;
        } else if self.state.mode == Mode::Cell {
            self.handle_cell_input(ctx, &is_pressed, &is_held)?;
        }

        return Ok(());
    }

    fn skip_to_cell(&mut self, ctx: &egui::Context) {
        let mouse_pos = self.state.device_state.query_pointer().coords;
        let mouse_pos = pos2(mouse_pos.0 as f32, mouse_pos.1 as f32);

        for (i, d) in self.state.displays.iter().enumerate() {
            if egui::Rect::from_min_size(d.pos, d.size).contains(mouse_pos) {
                let rel_pos = mouse_pos - d.pos;
                let region_size = vec2(d.size.x * 0.25, d.size.y * 0.25);
                let region_index = vec2(
                    (rel_pos.x / region_size.x).floor(),
                    (rel_pos.y / region_size.y).floor(),
                );
                self.state.region = (region_index.x + region_index.y * 4.0) as i32;

                let rel_pos = rel_pos
                    - vec2(
                        region_size.x * region_index.x,
                        region_size.y * region_index.y,
                    );
                let cell_size = vec2(region_size.x / 5.0, region_size.y / 3.0);
                let cell_index = vec2(
                    (rel_pos.x / cell_size.x).floor(),
                    (rel_pos.y / cell_size.y).floor(),
                );
                self.state.cell = (cell_index.x + cell_index.y * 5.0) as i32;

                self.state.mode = Mode::Cell;
                if i != self.state.current_display {
                    self.move_to_display(ctx, i);
                }
                self.state.mouse_key_down.clear();
                break;
            }
        }
    }
}

fn to_stroke(width: f32, col: Color) -> Stroke {
    let col = Color32::from_rgba_unmultiplied(col.0, col.1, col.2, col.3);
    Stroke::new(width, col)
}

fn to_col(col: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(col.0, col.1, col.2, col.3)
}

impl eframe::App for MyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }

    // Hack: egui::input doesn't send key down events for '+' keys for some reason. Investigation needed.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        for e in  &_raw_input.events {
            if let egui::Event::Key { key, physical_key, pressed, repeat, .. } = e {
                if let Some(k) = physical_key {
                    if *k == Key::Equals && *key == Key::Equals && *pressed == false && *repeat == false {
                        _raw_input.events.push(egui::Event::Key{
                            key: Key::Plus,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: Default::default(),
                        });
                        return;
                    }
                }
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        if let Err(input_err) = self.handle_input(ctx) {
            println!("Failed to manipluate mouse: {input_err}");
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let painter = ui.painter();
                let ref display = self.state.displays[self.state.current_display];
                let origin = Pos2::ZERO - display.offset;
                let style = &self.state.config.style;

                let region_line1_stroke = to_stroke(5.0, style.region_line1);
                let region_line2_stroke = to_stroke(3.0, style.region_line2);

                let region_size = vec2(display.size.x * 0.25, display.size.y * 0.25);
                let cell_size = vec2(region_size.x / 5.0, region_size.y / 3.0);

                if self.state.mode == Mode::Screen {
                    // Draw screen borders
                    let screen_border = Rect::from_min_size(origin, display.size).shrink(5.0);
                    painter.rect_stroke(screen_border, Rounding::ZERO, region_line1_stroke);
                    painter.rect_stroke(screen_border, Rounding::ZERO, region_line2_stroke);

                    let region_grid_line1_stroke = to_stroke(1.5, style.region_grid_line1);
                    let region_grid_line2_stroke = to_stroke(1.5, style.region_grid_line2);

                    // Draw horizontal lines
                    let horizontal_line_count = 12;
                    for i in 1..horizontal_line_count {
                        let percentage = i as f32 / horizontal_line_count as f32;
                        let left = origin + vec2(0.0, display.size.y * percentage);
                        let right = origin + vec2(display.size.x, display.size.y * percentage);

                        painter.line_segment([left, right], region_grid_line1_stroke);
                        painter.line_segment([left, right], region_grid_line2_stroke);
                    }

                    // Draw vertical lines
                    let vertical_line_count = 20;
                    for i in 1..vertical_line_count {
                        let percentage = i as f32 / vertical_line_count as f32;
                        let top = origin + vec2(display.size.x * percentage, 0.0);
                        let btm = origin + vec2(display.size.x * percentage, display.size.y);

                        painter.line_segment([top, btm], region_grid_line1_stroke);
                        painter.line_segment([top, btm], region_grid_line2_stroke);
                    }

                    // Draw region stripes
                    for i in 0..4 {
                        let rect = egui::Rect::from_min_size(
                            origin + vec2(0.0, i as f32 * region_size.y),
                            vec2(display.size.x, region_size.y),
                        );
                        let color = if i % 2 == 0 {
                            self.state.config.style.left_grid.clone()
                        } else {
                            self.state.config.style.right_grid.clone()
                        };

                        painter.rect(rect, Rounding::ZERO, to_col(color), Stroke::NONE);
                    }

                    let black_font = egui::FontId::new(60.0, egui::FontFamily::Proportional);
                    let white_font = egui::FontId::new(60.0, egui::FontFamily::Proportional);

                    let region_line1_stroke = to_stroke(2.0, style.region_line1);
                    let region_line2_stroke = to_stroke(1.0, style.region_line2);
                    for (i, key) in self.state.config.key_bindings.region.iter().enumerate() {
                        let region_x = (i % 4) as f32;
                        let region_y = (i / 4) as f32;

                        let text_pos = origin
                            + vec2(
                                (region_x + 0.5) * region_size.x,
                                (region_y + 0.5) * region_size.y,
                            );

                        // Draw region text
                        for i in 0..9 {
                            painter.text(
                                text_pos
                                    + vec2(((i % 3) - 1) as f32 * 3.0, ((i / 3) - 1) as f32 * 3.0),
                                Align2::CENTER_CENTER,
                                key.name(),
                                black_font.clone(),
                                Color32::BLACK,
                            );
                        }
                        painter.text(
                            text_pos,
                            Align2::CENTER_CENTER,
                            key.name(),
                            white_font.clone(),
                            Color32::WHITE,
                        );

                        // Draw region outline
                        let rect_pos =
                            origin + vec2(region_x * region_size.x, region_y * region_size.y);
                        painter.rect_stroke(
                            Rect::from_min_size(rect_pos, region_size),
                            Rounding::ZERO,
                            region_line1_stroke,
                        );
                        painter.rect_stroke(
                            Rect::from_min_size(rect_pos, region_size),
                            Rounding::ZERO,
                            region_line2_stroke,
                        );
                    }
                } else if self.state.mode == Mode::Narrow {
                    let origin = origin
                        + vec2(
                            region_size.x * (self.state.region % 4) as f32,
                            region_size.y * (self.state.region / 4) as f32,
                        );

                    // Draw region background
                    let right_color = to_col(style.right_grid);
                    let right_rect =
                        egui::Rect::from_min_size(origin, vec2(region_size.x, region_size.y));
                    painter.rect(right_rect, Rounding::ZERO, right_color, Stroke::NONE);

                    // Draw cell vertical lines
                    for i in 0..6 {
                        let i = i as f32;
                        let start = origin + vec2(i * cell_size.x, 0.0);
                        let end = origin + vec2(i * cell_size.x, region_size.y);
                        painter.line_segment([start, end], region_line1_stroke);
                        painter.line_segment([start, end], region_line2_stroke);
                    }

                    // Draw cell horizontal lines
                    for i in 0..4 {
                        let i = i as f32;
                        let start = origin + vec2(0.0, i * cell_size.y);
                        let end = origin + vec2(region_size.x, i * cell_size.y);
                        painter.line_segment([start, end], region_line1_stroke);
                        painter.line_segment([start, end], region_line2_stroke);
                    }

                    // Draw cell text
                    let black_font = egui::FontId::new(27.0, egui::FontFamily::Proportional);
                    let white_font = egui::FontId::new(20.0, egui::FontFamily::Proportional);
                    let text_offset = 6;
                    for i in 0..3 {
                        let pos = origin + vec2((i as f32 + 1.5) * cell_size.x, cell_size.y * 1.5);
                        let text = self.state.config.key_bindings.grid[text_offset + i].name();

                        for j in 0..9 {
                            painter.text(
                                pos
                                    + vec2(((j % 3) - 1) as f32 * 1.5, ((j / 3) - 1) as f32 * 1.5),
                                Align2::CENTER_CENTER,
                                text,
                                black_font.clone(),
                                Color32::BLACK,
                            );
                        }

                        painter.text(
                            pos,
                            Align2::CENTER_CENTER,
                            text,
                            white_font.clone(),
                            Color32::WHITE,
                        );
                    }
                } else if self.state.mode == Mode::Cell {
                    let origin = origin
                        + vec2(
                            region_size.x * (self.state.region % 4) as f32,
                            region_size.y * (self.state.region / 4) as f32,
                        )
                        + vec2(
                            cell_size.x * (self.state.cell % 5) as f32,
                            cell_size.y * (self.state.cell / 5) as f32,
                        );

                    // Draw cell borders
                    let cell_border = Rect::from_min_size(origin, cell_size).shrink(5.0);
                    painter.rect_stroke(cell_border, Rounding::ZERO, region_line1_stroke);
                    painter.rect_stroke(cell_border, Rounding::ZERO, region_line2_stroke);

                    // Draw cell background
                    let rect = egui::Rect::from_min_size(origin, cell_size);
                    painter.rect(rect, Rounding::ZERO, to_col(style.right_grid), Stroke::NONE);
                }

                let color = Color32::from_rgba_premultiplied(28, 92, 48, 120);
                let rect = egui::Rect::from_two_pos(pos2(0.0, 0.0), pos2(50.0, 50.0));
                painter.rect(rect, Rounding::ZERO, color, Stroke::new(0.0, color));

                ctx.send_viewport_cmd(ViewportCommand::Focus);
                ctx.request_repaint();
            });
    }
}
