//! simple widgets for the lock screen

use chrono::Local;
use egui::{RichText, Widget};
use std::time::Duration;

/// a simple clock widget, with time and date.
/// 
/// # Example
/// ```
/// # use lockrs::widgets::Clock;
/// # use egui::Color32;
/// 
/// Clock::new()
///      .time_style(|rich_text| rich_text.size(81.0).color(Color32::BLACK))
///      .date_style(|rich_text| rich_text.size(27.0).color(Color32::BLACK));
/// ```
pub struct Clock {
    pub time_style: Box<dyn FnOnce(RichText) -> RichText>,
    pub date_style: Box<dyn FnOnce(RichText) -> RichText>,
}

impl Clock {
    /// Create a new [`Clock`], with default size 30 
    /// for the time text, and 10 for the date
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the modifier function for the time rich text.
    pub fn time_style(mut self, f: impl FnOnce(RichText) -> RichText + 'static) -> Self {
        self.time_style = Box::new(f);
        self
    }

    /// Set the modifier function for the date rich text.
    pub fn date_style(mut self, f: impl FnOnce(RichText) -> RichText + 'static) -> Self {
        self.date_style = Box::new(f);
        self
    }
}

impl Widget for Clock {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.ctx().request_repaint_after(Duration::from_secs(60));

        let now = Local::now();
        let date = now.format("%A %_d/%m/%y").to_string();
        let time = now.format("%R").to_string();

        let time_text: egui::WidgetText = ((self.time_style)(RichText::new(time))).into();
        let date_text: egui::WidgetText = ((self.date_style)(RichText::new(date))).into();

        let time_galley = time_text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        let date_galley = date_text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            f32::INFINITY,
            egui::TextStyle::Body,
        );

        let width = time_galley.size().x.max(date_galley.size().x);
        let height = time_galley.size().y + date_galley.size().y + ui.spacing().item_spacing.y;

        ui.allocate_ui_with_layout(
            egui::vec2(width, height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(time_galley);
                ui.label(date_galley);
            },
        )
        .response
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            time_style: Box::new(|t| t.size(30.0)),
            date_style: Box::new(|t| t.size(10.0)),
        }
    }
}