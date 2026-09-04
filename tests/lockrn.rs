use egui::{Color32, Image, include_image};
use lockrs::prelude::*;

use egui_alignments::center_vertical;

fn bg_image() -> egui::ImageSource<'static> {
    include_image!("../wallhaven-sails.jpg")
}

#[test]
fn lockrn() {
    let mut app = App::init();

    let mut password = String::new();

    app.ui(|_output_name, ui, exit| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                Image::new(bg_image()).paint_at(ui, ui.ctx().content_rect());

                center_vertical(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(
                            widgets::Clock::new()
                                .time_style(|rich_text| rich_text.size(81.0).color(Color32::BLACK))
                                .date_style(|rich_text| rich_text.size(27.0).color(Color32::BLACK)),
                        );

                        ui.add(
                            egui::TextEdit::singleline(&mut password)
                                .desired_width(300.0)
                                .hint_text("Password...")
                                .horizontal_align(egui::Align::Center)
                                .password(true),
                        );

                        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                            *exit = TryExit::Force
                        } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            *exit = TryExit::PasswdCheck(password.clone())
                        }
                    })
                })
            });
    });
}
