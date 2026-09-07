use egui::{Color32, Image, include_image};
use lockrs::{prelude::*, widgets::{FailureTextLocation, PasswordTextEdit, Uptime}};

use egui_alignments::center_vertical;

fn bg_image() -> egui::ImageSource<'static> {
    include_image!("../wallhaven-sails.jpg")
}

#[test]
fn lockrn() {
    let mut app = App::init();

    let mut password = String::new();
    let mut has_failed = false;

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
                            PasswordTextEdit::new(&mut password, &mut has_failed)
                                .modify_textedit(|t| {
                                    t.desired_width(300.0)
                                        .horizontal_align(egui::Align::Center)
                                })
                                .modify_failure_text(|t| t.size(20.0))
                                .set_fail_text_location(FailureTextLocation::Bottom(egui::Align::Center))
                        );

                        ui.add(Uptime::new());

                        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                            *exit = TryExit::Force
                        } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            *exit = TryExit::PasswdCheck(password.clone());
                            has_failed = true; // if it doesn't pass immediately, it failed and won't come off again
                        }
                    })
                })
            });
    });
}