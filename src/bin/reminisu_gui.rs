#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> eframe::Result {
    eframe::run_native(
        "Reminisu • Economy control center",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!("../../app-icon.png"))
                        .expect("bundled app icon is valid"),
                )
                .with_inner_size([1280.0, 850.0])
                .with_min_inner_size([850.0, 600.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(reminisu_economy::gui::EconomyApp::new(cc)))),
    )
}
