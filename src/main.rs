use display_info::DisplayInfo;
use enigo::{Button, Enigo, Keyboard, Mouse, Settings};

use eframe::egui;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use eframe::egui::{ViewportCommand};
use egui::{Key, ScrollArea};


use device_query::{DeviceQuery, DeviceState, Keycode};

fn main() -> eframe::Result {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.move_mouse(2000, 500, enigo::Coordinate::Abs).unwrap();

    let display_infos = DisplayInfo::all().unwrap();
    for display_info in display_infos {
        println!("display_info {display_info:?}");

        let w = display_info.width as i32;
        let h = display_info.height as i32;
        let x = display_info.x + w / 2;
        let y = display_info.y + h / 2;
        //     enigo.move_mouse(x, y, enigo::Coordinate::Abs).unwrap();
        //    std::thread::sleep(std::time::Duration::from_secs(3));
    }
    // enigo.button(Button::Left, enigo::Direction::Click).unwrap();
    //   enigo.text("Hello World! here is a lot of text  ❤️").unwrap();


    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false) // Hide the OS-specific "chrome" around the window
            .with_mouse_passthrough(true)
            .with_always_on_top()
            .with_transparent(true),

        ..Default::default()
    };
    eframe::run_native(
        "Custom window frame", // unused title
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}


#[derive(Default)]
struct MyApp {}

impl eframe::App for MyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {


        custom_window_frame(ctx, "egui with custom frame", |ui| {
            ui.label("This is just the contents of the window.");
            ui.horizontal(|ui| {
                ui.label("egui theme:");
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        // let device_state = DeviceState::new();
        // let keys: Vec<Keycode> = device_state.get_keys();

        // println!("Window active!");
        // if keys.contains(&Keycode::Q) {
        //     println!("Q is pressed!");
        //     ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(1500.0,120.0)));
        //     let mut enigo = Enigo::new(&Settings::default()).unwrap();
        //     enigo.button(Button::Left, enigo::Direction::Press).unwrap();
        // }
        // if keys.contains(&Keycode::W) {
        //     println!("W is pressed!");
        //     ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(900.0,240.0)));

        //     let mut enigo = Enigo::new(&Settings::default()).unwrap();
        //     enigo.button(Button::Left, enigo::Direction::Release).unwrap();
        // }
        // if keys.contains(&Keycode::E) {
        //     println!("E is pressed!");
        //     ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(300.0,360.0)));
        // }
    }
}

fn custom_window_frame(ctx: &egui::Context, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    use egui::{CentralPanel, UiBuilder};
    let panel_frame = egui::Frame::none();

    CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
        let app_rect = ui.max_rect();

        let title_bar_height = 32.0;
        let title_bar_rect_max_y = app_rect.min.y + title_bar_height;

        // Add the contents:
        let content_rect = {
            let mut rect = app_rect;
            rect.min.y = title_bar_rect_max_y;
            rect
        }
        .shrink(4.0);
        let mut content_ui = ui.new_child(UiBuilder::new().max_rect(content_rect));
        add_contents(&mut content_ui);
    });
}