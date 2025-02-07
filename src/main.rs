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
    left_region: [String; 3],
    right_region: [String; 3],
    prev_screen: String,
    next_screen: String,

    left_grid: [String; 15],
    right_grid: [String; 15],
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
        let mut left_region: [Key; 3] = [Key::Space; 3];
        let mut right_region: [Key; 3] = [Key::Space; 3];
        for (i, val) in self.left_region.iter().enumerate() {
            left_region[i] = to_keycode(val);
        }
        for (i, val) in self.right_region.iter().enumerate() {
            right_region[i] = to_keycode(val);
        }

        let mut left_grid: [Key; 15] = [Key::Space; 15];
        for (i, val) in self.left_grid.iter().enumerate() {
            left_grid[i] = to_keycode(val);
        }

        let mut right_grid: [Key; 15] = [Key::Space; 15];
        for (i, val) in self.right_grid.iter().enumerate() {
            right_grid[i] = to_keycode(val);
        }

        ScreenModeBindings {
            regions: [
                left_region[0], left_region[1], left_region[2],
                right_region[0], right_region[1], right_region[2]
            ],
            prev_screen: to_keycode(&self.prev_screen),
            next_screen: to_keycode(&self.next_screen),
            left_grid,
            right_grid,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenModeBindings {
    regions: [Key; 6],
    prev_screen: Key,
    next_screen: Key,

    left_grid: [Key; 15],
    right_grid: [Key; 15],
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonConfig {
    primary_offset_x: i32,
    primary_offset_y: i32,
    screen_mode_bindings: JsonScreenModeBindings,
}

impl JsonConfig {
    fn transform(&self) -> Config {
        Config {
            primary_offset_x: self.primary_offset_x,
            primary_offset_y: self.primary_offset_y,
            screen_mode_bindings: self.screen_mode_bindings.transform(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Config {
    primary_offset_x: i32,
    primary_offset_y: i32,
    screen_mode_bindings: ScreenModeBindings,
}

#[atomic_enum::atomic_enum]
#[derive(PartialEq)]
enum Mode {
    Screen,
    Narrow,
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
        update_thread: None,
        state: SharedState {
            displays,
            current_display: Arc::new(AtomicUsize::new(initial_display_idx)),
            config,
            mode: Arc::new(AtomicMode::new(Mode::Screen)),
            region: Arc::new(AtomicUsize::new(0)),
        },
    };

    eframe::run_native(
        "Custom window frame", // unused title
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

struct MyApp {
    update_thread: Option<JoinHandle<()>>,
    state: SharedState,
}

struct SharedState {
    displays: Vec<Display>,
    current_display: Arc<AtomicUsize>,
    config: Config,
    mode: Arc<AtomicMode>,
    region: Arc<AtomicUsize>,
}

impl MyApp {
    fn handle_input(&mut self, ctx: &egui::Context) {

        let state = &self.state;
        let input = ctx.input(|i| i.clone());
        let is_pressed = |&k| -> bool {
            if k == Key::F20 {
                return input.modifiers.shift;
            } else if k == Key::F21 {
                return input.modifiers.ctrl;
            }
            input.key_pressed(k)
        };

        if is_pressed(&state.config.screen_mode_bindings.prev_screen) {
            let mut next_display = state.current_display.load(Ordering::Acquire);
            next_display = if next_display == 0 { state.displays.len() - 1 } else { next_display - 1 };
            move_to_display(&ctx, &state, next_display);

        } else if is_pressed(&state.config.screen_mode_bindings.next_screen) {
            let next_display = state.current_display.load(Ordering::Acquire) + 1;
            move_to_display(&ctx, &state, next_display);
        }

        for (i, key) in state.config.screen_mode_bindings.regions.iter().enumerate() {
            if is_pressed(key) {
                println!("Is pressed {key:#?}");
                state.region.store(i, Ordering::Relaxed);
                state.mode.store(Mode::Narrow, Ordering::Relaxed);
                ctx.request_repaint();
                break;
            }
        }

        for (i, key) in state.config.screen_mode_bindings.left_grid.iter().enumerate() {
            if is_pressed(key) {
                println!("Left grid pressed {key:#?}");

                let display = state.displays[state.current_display.load(Ordering::Acquire)];
                let region = state.region.load(Ordering::Acquire);

                let mut pos = display.pos;
                if region >= 3 {
                    pos.x += display.size.x * 0.5;
                }
                pos.y += display.size.y * 0.333 * (region % 3) as f32;
                let col = i % 5;
                let row = i / 5;

                let region_y = display.size.y * 0.333;
                let cell_y = region_y / 3.0;
                print!("region y {region_y} cell_y {cell_y}");

                let cell_size = vec2( display.size.x * 0.1 * 0.5, cell_y);
                let half_cell_size = cell_size * 0.5;

                pos.x += col as f32 * cell_size.x;
                pos.y += row as f32 * cell_size.y;

                pos += half_cell_size;

                let mut enigo = Enigo::new(&Settings::default()).unwrap();
                enigo.move_mouse(pos.x as i32, pos.y as i32, enigo::Coordinate::Abs);
            }
        }

        for (i, key) in state.config.screen_mode_bindings.right_grid.iter().enumerate() {
            if is_pressed(key) {
                println!("Right grid pressed {key:#?}");
                
                let display = state.displays[state.current_display.load(Ordering::Acquire)];
                let region = state.region.load(Ordering::Acquire);

                let mut pos = display.pos;
                if region >= 3 {
                    pos.x += display.size.x * 0.5;
                }
                pos.x += display.size.x * 0.25;
                pos.y += display.size.y * 0.333 * (region % 3) as f32;
                let col = i % 5;
                let row = i / 5;

                let region_y = display.size.y * 0.333;
                let cell_y = region_y / 3.0;
                print!("region y {region_y} cell_y {cell_y}");

                let cell_size = vec2( display.size.x * 0.1 * 0.5, cell_y);
                let half_cell_size = cell_size * 0.5;

                pos.x += col as f32 * cell_size.x;
                pos.y += row as f32 * cell_size.y;

                pos += half_cell_size;

                let mut enigo = Enigo::new(&Settings::default()).unwrap();
                enigo.move_mouse(pos.x as i32, pos.y as i32, enigo::Coordinate::Abs);
            }
        }

        if is_pressed(&Key::Escape) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
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
        //  ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));

//        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        if self.update_thread.is_none() {
            self.spawn_thread(ctx.clone());
        }

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
            let region = self.state.region.load(Ordering::Acquire);
            let display = self.state.current_display.load(Ordering::Acquire);
            let ref display = self.state.displays[display];
            let origin = Pos2::ZERO - display.offset;

            let light_gray = Color32::from_rgba_premultiplied(200, 200, 200, 120);
            let dark_gray = Color32::from_rgba_premultiplied(0, 0, 0, 120);
            let light_gray_stroke = Stroke::new(5.0, light_gray);
            let dark_gray_stroke = Stroke::new(3.0, dark_gray);

            let mode = self.state.mode.load(Ordering::Acquire);
            if mode == Mode::Screen {
                let feather = 5.0;
                let edges = vec![
                    (
                        origin + vec2(feather, feather),
                        origin + vec2(display.size.x - feather, feather),
                    ),
                    (
                        origin + vec2(feather, display.size.y - feather),
                        origin + vec2(display.size.x - feather, display.size.y - feather),
                    ),
                    (
                        origin + vec2(feather, feather),
                        origin + vec2(feather, display.size.y - feather),
                    ),
                    (
                        origin + vec2(display.size.x - feather, feather),
                        origin + vec2(display.size.x - feather, display.size.y - feather),
                    ),
                ];

                for edge in edges {
                    painter.line_segment([edge.0, edge.1], light_gray_stroke);
                    painter.line_segment([edge.0, edge.1], dark_gray_stroke);
                }
            } else {
                let mut origin = origin;
                if region < 3 {
                    origin += vec2(display.size.x * 0.0, display.size.y * 0.333 * region as f32);
                } else {
                    origin += vec2(display.size.x * 0.5, display.size.y * 0.333 * (region-3) as f32);
                }

                let size = vec2(display.size.x * 0.5, display.size.y * 0.333);
                for i in 0..11 {
                    let i = i as f32;
                    let start = origin + vec2(size.x * i * 0.1, 0.0);
                    let end = origin + vec2(size.x * i * 0.1, size.y);
                    painter.line_segment([start, end], light_gray_stroke);
                    painter.line_segment([start, end], dark_gray_stroke);
                }

                for i in 0..4 {
                    let i = i as f32;
                    let start = origin + vec2(0.0, size.y * i * 0.333);
                    let end = origin + vec2(size.x, size.y * i * 0.333);
                    painter.line_segment([start, end], light_gray_stroke);
                    painter.line_segment([start, end], dark_gray_stroke);
                }

                let left_color = Color32::from_rgba_premultiplied(252, 118, 106, 120);
                let left_rect =
                    egui::Rect::from_two_pos(origin, origin + vec2(size.x * 0.5, size.y));
                painter.rect(left_rect, Rounding::ZERO, left_color, Stroke::NONE);

                let right_color = Color32::from_rgba_premultiplied(91, 132, 177, 120);
                let right_rect = egui::Rect::from_two_pos(
                    origin + vec2(size.x * 0.5, 0.0),
                    origin + vec2(size.x, size.y),
                );
                painter.rect(right_rect, Rounding::ZERO, right_color, Stroke::NONE);
            }

            let color = Color32::from_rgba_premultiplied(28, 92, 48, 120);
            let rect = egui::Rect::from_two_pos(pos2(0.0, 0.0), pos2(50.0, 50.0));
            painter.rect(rect, Rounding::ZERO, color, Stroke::new(0.0, color));

            ctx.request_repaint();
        });
    }
}

fn move_to_display(ctx: &egui::Context, state: &SharedState, display_idx: usize) {
    let display_idx = display_idx % state.displays.len();
    state.current_display.store(display_idx, Ordering::Relaxed);

    let ref display = state.displays[display_idx];
    let pos = display.pos + display.offset;
    let size = display.size - display.offset;

    ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
    ctx.request_repaint();
}

fn main_logic(
    ctx: egui::Context,
    state: SharedState,
) {
    println!("Start of main logic!");

    let device_state = DeviceState::new();
    let mut prev_keys = device_state.get_keys();
    loop {
        // let keys: Vec<Keycode> = device_state.get_keys();
        // let is_pressed = |k| -> bool {
        //     //     if k == &Keycode::LShift {
        //     //         let b = ctx.input(|i| i.modifiers.shift);
        //     //   //      println!("LShift {b}");
        //     //         return b;
        //     //     }
        //     //     if k == &Keycode::BackSlash {
        //     //         return ctx.input(|i| i.key_pressed(Key::Backslash));
        //     //     }
        //     keys.contains(k) && !prev_keys.contains(k)
        // };

        // if is_pressed(&state.config.screen_mode_bindings.prev_screen) {
        //     let mut next_display = state.current_display.load(Ordering::Acquire);
        //     next_display = if next_display == 0 { state.displays.len() - 1 } else { next_display - 1 };
        //     move_to_display(&ctx, &state, next_display);

        // } else if is_pressed(&state.config.screen_mode_bindings.next_screen) {
        //     let next_display = state.current_display.load(Ordering::Acquire) + 1;
        //     move_to_display(&ctx, &state, next_display);
        // }

        // for (i, key) in state.config.screen_mode_bindings.regions.iter().enumerate() {
        //     if is_pressed(key) {
        //         println!("Is pressed {key:#?}");
        //         state.region.store(i, Ordering::Relaxed);
        //         state.mode.store(Mode::Narrow, Ordering::Relaxed);
        //         ctx.request_repaint();
        //         break;
        //     }
        // }

        // if is_pressed(&Keycode::A) {
        //     println!("Is pressed a")
        // }
        // if is_pressed(&Keycode::S) {
        //     println!("Is pressed S")
        // }
        // if is_pressed(&Keycode::D) {
        //     println!("Is pressed d")
        // }
        // if is_pressed(&Keycode::F) {
        //     println!("Is pressed f")
        // }

        // if is_pressed(&Keycode::Escape) {
        //     ctx.send_viewport_cmd(ViewportCommand::Close);
        // }

        // prev_keys = keys;
        // println!("{}", now.unwrap().as_millis());
        // if keys.contains(&Keycode::Enter) {
        //     if one_flag == false {
        //         println!("Enter is pressed!");
        //         // let mut enigo = Enigo::new(&Settings::default()).unwrap();
        //         // enigo.button(Button::Left, enigo::Direction::Press).unwrap();

        //         one_flag = true;
        //         let mut d = state.current_display.load(Ordering::Acquire);
        //         d = (d + 1) % state.displays.len();
        //         state.current_display.store(d, Ordering::Relaxed);

        //         let ref display = state.displays[d];
        //         let pos = display.pos + display.offset;
        //         let size = display.size - display.offset;

        //         ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        //         ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        //         ctx.request_repaint();
        //     }
        // } else {
        //     one_flag = false;
        // }

        // if keys.contains(&Keycode::Tab) {
        //     if two_flag == false {
        //         println!("Tab is pressed!");
        //         state.mode.store(Mode::Left, Ordering::Relaxed);
        //         ctx.request_repaint();
        //         two_flag = true;
        //     }
        // } else {
        //     two_flag = false;
        // }
        // if keys.contains(&Keycode::W) {
        //     println!("W is pressed!");
        //     ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
        //         900.0, 240.0,
        //     )));

        //     let mut enigo = Enigo::new(&Settings::default()).unwrap();
        //     enigo
        //         .button(Button::Left, enigo::Direction::Release)
        //         .unwrap();

        //     ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        // }
        // if keys.contains(&Keycode::E) {
        //     println!("E is down!");
        //     if two_flag == false {
        //         println!("E is pressed!");

        //         let mut enigo = Enigo::new(&Settings::default()).unwrap();
        //         enigo.button(Button::Left, enigo::Direction::Click).unwrap();

        //         ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        //         two_flag = true;
        //     }
        // } else {
        //     two_flag = false;
        // }
        //  std::thread::sleep(std::time::Duration::from_millis(1));
        //            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(300.0,360.0)));
    }
}

impl MyApp {
    fn spawn_thread(&mut self, ctx: egui::Context) {
        let displays = self.state.displays.clone();
        let current_display = self.state.current_display.clone();
        let config = self.state.config.clone();
        let mode = self.state.mode.clone();
        let region = self.state.region.clone();
        let state = SharedState {
            displays,
            current_display,
            config,
            mode,
            region,
        };
        let handle =
            std::thread::spawn(move || main_logic(ctx, state));
        self.update_thread = Some(handle);
    }
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
