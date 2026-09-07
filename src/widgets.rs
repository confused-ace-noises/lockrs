//! simple widgets for the lock screen

use chrono::Local;
use egui::{Align, Layout, RichText, TextEdit, Widget, WidgetText};
use std::{process::Command, time::Duration};

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

/// Location for the failure to login text in a [`PasswordTextEdit`]
pub enum FailureTextLocation {
    /// Failure text on top of the input box; the additional [`Align`]
    /// field provided specifies whether the failure text should be aligned
    /// to the left edge of the input box ([`Align::Min`]), to the right edge ([`Align::Max`])
    /// or be centered ([`Align::Center`]) 
    Top(Align),
    
    /// Failure text below the input box; the additional [`Align`]
    /// field provided specifies whether the failure text should be aligned
    /// to the left edge of the input box ([`Align::Min`]), to the right edge ([`Align::Max`])
    /// or be centered ([`Align::Center`])
    Bottom(Align),
    
    /// Failure text is on the left of the input box
    Left,

    /// Failure text is on the right of the input box
    Right
}

/// A [`TextEdit`] that also has a failure text, aligned via [`PasswordTextEdit::set_fail_text_location`].
/// 
/// 
/// # Example
/// ```
/// let mut passwd_string = String::new();
/// let mut has_failed = false;
/// 
/// let passwd_text_edit = PasswordTextEdit::new(&mut passwd_string, &mut has_failed)
///     .modify_textedit(|t| t.desired_width(300.0).horizontal_align(egui::Align::Center))
///     .modify_failure_text(|t| t.size(20.0))
///     .set_fail_text_location(FailureTextLocation::Bottom(egui::Align::Center));
/// ```
pub struct PasswordTextEdit<'t> {
    textedit: TextEdit<'t>,
    failure_text: RichText,
    has_failed: &'t mut bool,
    loc: FailureTextLocation,
}

impl<'t> PasswordTextEdit<'t> {
    /// create a new [`PasswordTextEdit`]. 
    /// By default, this has:
    /// textedit              : `TextEdit::singleline(input).password(true).hint_text("Password...")`
    /// failure_text          : `RichText::new("login failed")`
    /// failure_text_location : `FailureTextLocation::Bottom(egui::Align::Center)`
    pub fn new(input: &'t mut String, has_failed: &'t mut bool) -> Self {
        Self {
            textedit: TextEdit::singleline(input).password(true).hint_text("Password..."),
            failure_text: RichText::new("login failed"),
            has_failed,
            loc: FailureTextLocation::Bottom(egui::Align::Center)
        }
    }

    /// Modifies the inner [`TextEdit`]
    pub fn modify_textedit(mut self, f: impl FnOnce(TextEdit<'t>) -> TextEdit<'t>) -> Self {   
        self.textedit = f(self.textedit);
        self
    }

    /// Modifies the failure text
    pub fn modify_failure_text(mut self, f: impl FnOnce(RichText) -> RichText) -> Self {   
        self.failure_text = f(self.failure_text);
        self
    }

    /// Modifies the inner [`FailureTextLocation`]; if you don't need to inspect it,
    /// and you just need to set it, use [`PasswordTextEdit::set_fail_text_location`].
    pub fn modify_fail_text_location(mut self, f: impl FnOnce(FailureTextLocation) -> FailureTextLocation) -> Self {   
        self.loc = f(self.loc);
        self
    }

    /// Sets the inner [`FailureTextLocation`]
    pub fn set_fail_text_location(mut self, loc: FailureTextLocation) -> Self {
        self.loc = loc;
        self
    }
}

impl<'t> Widget for PasswordTextEdit<'t> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let layout = match self.loc {
            FailureTextLocation::Top(_)
                | FailureTextLocation::Bottom(_) => Layout::top_down(egui::Align::Min),
            FailureTextLocation::Left 
                | FailureTextLocation::Right  => Layout::left_to_right(egui::Align::Min),
        };

        ui.with_layout(layout, |ui| {
            let failed_text_galley = Into::<WidgetText>::into(self.failure_text).into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                f32::INFINITY,
                egui::TextStyle::Body,
            );

            let failed_text_space = match self.loc {
                FailureTextLocation::Top(_) | FailureTextLocation::Bottom(_) => failed_text_galley.size().y,
                FailureTextLocation::Left | FailureTextLocation::Right => failed_text_galley.size().x,
            };
            
            if let FailureTextLocation::Top(align) = self.loc {
                if *self.has_failed {
                    ui.with_layout(Layout::top_down(align), |ui| ui.label(failed_text_galley.clone()));
                } else {
                    ui.add_space(failed_text_space);
                }
            } else if let FailureTextLocation::Left = self.loc {
                if *self.has_failed {
                    ui.label(failed_text_galley.clone());
                } else {
                    ui.add_space(failed_text_space);
                }
            }

            ui.add(self.textedit);

            if let FailureTextLocation::Bottom(align) = self.loc {
                if *self.has_failed {
                    ui.with_layout(Layout::top_down(align), |ui| ui.label(failed_text_galley.clone()));
                } else {
                    ui.add_space(failed_text_space);
                }
            } else if let FailureTextLocation::Right = self.loc {
                if *self.has_failed {
                    ui.label(failed_text_galley.clone());
                } else {
                    ui.add_space(failed_text_space);
                }
            }
        }).response
    }
}

/// A simple uptime display. By default, it displays
/// the uptime as `$hours hours, $minutes minutes` where `$hours`
/// and `$minutes` are replaced by the number of hours and minutes of
/// uptime respectively.
pub struct Uptime {
    last_uptime: String,
    text_style: Option<Box<dyn FnOnce(RichText) -> RichText>>,
    text_modify: Option<Box<dyn FnOnce(String) -> String>>,
}

impl Uptime {
    /// create a new [`Uptime`] display
    pub fn new() -> Self {
        Uptime { last_uptime: Self::uptime().unwrap_or("ERROR".to_string()), text_style: None, text_modify: None }
    }

    fn uptime() -> Option<String> {
        let res = Command::new("uptime").arg("-p").output().ok()?;
        
        let binding = String::from_utf8_lossy(&res.stdout);
        let (_, uptime) = binding.split_once(' ')?;

        Some(uptime.to_string())
    }

    /// modify the uptime string itself. the passed-in string is formatted
    /// with the default formatting. (see [`Uptime`] docs for the description of the default)
    pub fn modify_uptime_string(mut self, f: impl FnOnce(String) -> String + 'static) -> Self {
        self.text_modify = Some(Box::new(f));
        self
    }

    /// modifies the [`RichText`] with which the string is displayed
    pub fn modify_text_style(mut self, f: impl FnOnce(RichText) -> RichText + 'static) -> Self {
        self.text_style = Some(Box::new(f));
        self
    } 
}

impl Default for Uptime {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Uptime {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.ctx().request_repaint_after(Duration::from_secs(60));

        let modified_uptime = if let Some(f) = self.text_modify {
            f(self.last_uptime)
        } else {
            self.last_uptime
        };

        let text = if let Some(f) = self.text_style {
            f(RichText::new(modified_uptime))
        } else {
            RichText::new(modified_uptime)
        };

        ui.label(text)
    }
}

#[derive(Default)]
/// A simple label that shows the default network interface
pub struct DefaultNetworkInterface(Option<Box<dyn FnOnce(RichText) -> RichText + 'static>>);

impl DefaultNetworkInterface {
    /// Create a new [`DefaultNetworkInterface`]
    pub fn new() -> Self {
        Self(None)
    }

    /// Set the [`RichText`] style for the label
    pub fn set_text_style(mut self, f: impl FnOnce(RichText) -> RichText + 'static) -> Self {
        self.0 = Some(Box::new(f));
        self
    }

    fn interface() -> Option<String> {
        let output = Command::new("ip").args(["route", "show", "default"]).output().ok()?;
        
        let string = String::from_utf8_lossy(&output.stdout);
        let interface = string.split(' ').nth(4)?;

        Some(interface.to_string())
    }
}

impl Widget for DefaultNetworkInterface {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let interface = RichText::new(Self::interface().unwrap_or_else(|| String::from("ERROR")));

        let ingterface = if let Some(f) = self.0 {
            f(interface)
        } else {
            interface
        };

        ui.label(ingterface)
    }
}