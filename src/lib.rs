//! # Lockrs
//! ![MIT or Apache 2.0 Licensed](https://img.shields.io/badge/license-MIT_OR_Apache%202.0-blue?style=for-the-badge)
//! 
//! Lockrs is a library that allows egui to render to a lock screen through the `ext-session-lock-v1` protocol. It also allows for different output on different monitors.
//! 
//! ## Why?
//! Lockrs aims to be a scaffold for screen lockers, NOT a screen locker in of itself. You may also need to get into the fields of the library's types, if you need to do fancy stuff. 
//! So, the whole reason for this project's existence isn't to provide a ready-to-use lockscreen, but to be a way to make the most customizable lockscreen possible without going to down to protocol primitives.
//! 
//! ## How?
//! 
//! To use the library, you first need to initialize the `App` struct through the `init` method. This will connect to the compositor, handle the surfaces, initialize `wgpu` and `egui` and handle the low-level stuff, in general. Once you init the `App` struct, you can use the `ui` method to talk to `egui`.
//! 
//! Note: it's heavily recommended to compile this crate with `opt-level = 3`, because the image loading that egui does for the background, for example, is ***significanlty*** sped up (a few seconds to a few milliseconds). This may achieved by manually setting the `opt-level` for the desidered profile in the `Cargo.toml`, or compiling with the `--release` flag.
//! 
//! ## Example
//! This is my personal lock screen:
//! ```rs
//! use egui::{Color32, Image, include_image};
//! use lockrs::prelude::*;
//! 
//! // main
//! let mut app = App::init();
//! 
//! let mut password = String::new();
//! 
//! app.ui(|_output_name, ui, exit| {
//!     egui::CentralPanel::default()
//!         .frame(egui::Frame::NONE)
//!         .show(ui, |ui| {
//!             Image::new(include_image!("path/to/background")).paint_at(ui, ui.ctx().content_rect());
//! 
//!             center_vertical(ui, |ui| {
//!                 ui.vertical_centered(|ui| {
//!                     ui.add(
//!                         widgets::Clock::new()
//!                             .time_style(|rich_text| rich_text.size(81.0).color(Color32::BLACK))
//!                             .date_style(|rich_text| rich_text.size(27.0).color(Color32::BLACK)),
//!                     );
//! 
//!                     ui.add(
//!                         egui::TextEdit::singleline(&mut password)
//!                             .desired_width(300.0)
//!                             .hint_text("Password...")
//!                             .horizontal_align(egui::Align::Center)
//!                             .password(true),
//!                     );
//! 
//!                     if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
//!                         *exit = Action::Force
//!                     } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
//!                         *exit = Action::PasswdCheck(password.clone())
//!                     }
//!                 })
//!             })
//!         });
//! });
//! ```
//! 
//! this code will produce the following output:
//! ![screenshot of lockrs' output](.assets/screenshot.png)
//! 
//! ## Usage
//! To use this library, simply add it in the dependencies of your Rust project:
//! ```toml
//! # Cargo.toml
//! 
//! [dependencies]
//! lockrs = "0.3.0" # put latest version here
//! 
//! egui = "0.36.1"
//! ```
//! 
//! ### Updates
//! This library may be updated in the future, so if it does happen, the API will probably change a bit until it's in a more stable situation.
#[cfg(feature = "screenshot")]
use std::{cell::RefCell, path::PathBuf};
use std::{collections::VecDeque, ffi::c_void, ptr::NonNull, rc::Rc};

use egui::Ui;
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle
};
use wayland_client::{
    Connection, Proxy, WEnum, protocol::{
        wl_output::WlOutput, wl_seat::{Capability, WlSeat}, wl_surface::WlSurface,
    }
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use wgpu::{Surface as WgpuSurface, SurfaceTarget};
use crate::{state::{App, FrameContext, PointerEvent, worker::WorkerEvent}, utils::late::Late};

pub mod state;
pub mod utils;
pub mod widgets;

/// wayland seat representation.
pub struct Seat {
    /// wl_seat global
    pub wl_seat: WlSeat,
    /// seat capabilities, if any
    pub capabilities: Option<WEnum<Capability>>,
    /// name of this seat
    pub name: Option<String>,
}

/// in-library output representation, containing the 
/// data about the output global itself and the data to 
/// render to said output
pub struct Output {
    /// egui context for this output
    pub egui_context: Late<egui::Context>,
    /// pending egui events for this output
    pub events_to_flush: Vec<egui::Event>,
    /// pending pointer events for this output
    pub pointer_events: Vec<PointerEvent>,
    /// index of the last treated axis event in the pointer events
    pub last_pointer_axis_event: Option<usize>,
    /// wl_output global
    pub wl_output: WlOutput,
    /// information about the surface that occupies this output
    pub surface_info: Late<SurfaceInfo>,
    /// name of the output global
    pub name: u32,
    /// human-readable name of the output, passed to the function
    /// passed to [App::ui]. 
    pub display_name: Late<Rc<String>>,
    /// whether the ext_session_lock_surface_v1 surface has gotten a
    /// configure event
    pub configured: bool,
    pub is_focused: bool,
}

impl Output {
    /// create a new output that has the following fields uninitialized:
    /// - `egui_context`
    /// - `surface_info`
    /// - `display_name`
    pub fn new_uninit(wl_output: WlOutput, name: u32) -> Self {
        Self {
            egui_context: Late::uninit(),
            pointer_events: Vec::new(),
            events_to_flush: Vec::new(),
            last_pointer_axis_event: None,
            wl_output,
            surface_info: Late::uninit(),
            name,
            display_name: Late::uninit(),
            configured: false,
            is_focused: true,
        }
    }
}

/// information about the surface of an output
pub struct SurfaceInfo {
    pub surface: WlSurface,
    pub lock_surface: ExtSessionLockSurfaceV1,
    pub surface_handle: WaylandSurfaceH,
    pub width: Late<u32>,
    pub height: Late<u32>,
    pub pending_scaling: Late<f32>,
    pub wgpu_surface: Late<WgpuSurface<'static>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// thin wrapper around [`WaylandDisplayHandle`]
pub struct WaylandDisplayH(WaylandDisplayHandle);

impl WaylandDisplayH {
    // this can be meaningfully 'static because the backend of the Connection will be alive for the
    // program's duration
    /// create a new [`WaylandDisplayH`] from a compositor connection.
    pub fn new(conn: &Connection) -> Self {
        Self(WaylandDisplayHandle::new(
            NonNull::new(conn.backend().display_ptr() as *mut c_void).unwrap(),
        ))
    }
}


impl HasDisplayHandle for WaylandDisplayH {
    /// gets a 'static borrow to the [`DisplayHandle`].s
    /// this is meaningfully 'static because the backend of the connection used
    /// lives for the entirety of the program
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'static>, raw_window_handle::HandleError> {
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(self.0)) })
    }
}

unsafe impl Send for WaylandDisplayH {}
unsafe impl Sync for WaylandDisplayH {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// thin wrapper around [`WaylandWindowHandle`]
pub struct WaylandSurfaceH(WaylandWindowHandle);

impl WaylandSurfaceH {
    /// create a new [`WaylandSurfaceH`] from a [`WlSurface`]
    pub fn new(wl_surface: &WlSurface) -> Self {
        Self(WaylandWindowHandle::new(
            NonNull::new(wl_surface.id().as_ptr() as *mut c_void).unwrap(),
        ))
    }
}

impl HasWindowHandle for WaylandSurfaceH {
    // this is probably 'static, becuase the program only creates one surface
    // in the duration of the program and never destroys until its end, but i'm
    // sure i'll end up shooting myself in the foot 
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'static>, raw_window_handle::HandleError> {
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Wayland(self.0)) })
    }
}

unsafe impl Send for WaylandSurfaceH {}
unsafe impl Sync for WaylandSurfaceH {}


/// describes an action for the lockscreen:
/// - [`Action::None`]: don't try to exit the lockscreen.
/// - [`Action::ForceExit`]: force the lockscreen to exit without doing a password check.
/// - [`Action::PasswdCheck`]: pass a [`String`]; if it matches the password,
///   the lockscreen will exit, otherwise assume that the password check failed.
/// - [`Action::TakeScreenshot`]: pass a [`PathBuf`], and a screenshot of the lockscreen. Only available 
///   with the `screenshot` feature.
pub enum Action {
    None,
    #[cfg(feature = "screenshot")]
    TakeScreenshot(PathBuf),
    ForceExit, 
    PasswdCheck(String),
}

impl App {
    
    /// Run egui.
    /// 
    /// The `output_fn` has three arguments: 
    /// 1. the wayland name for the output being rendered to currently. 
    ///    This also means that you can render different things for different outputs
    ///    by matching on their names.
    /// 2. the egui ui handle.
    /// 3. a mutable reference to a [`Action`]. This is by defualt [`Action::None`]. 
    ///    it to control how the lock screen should exit (note: this pointer points to a different
    ///    Action for each output. If different [`Action::PasswdCheck`] are set for 
    ///    different output passes, only the last one will be considered.)
    pub fn ui<F: for<'a> FnMut(&String, &'a mut Ui, &mut Action)>(&mut self, mut output_fn: F) {
        let mut should_break: bool;
        let mut should_auth: Option<String>;
        let mut doing_auth = false;
        let mut should_recreate_surface = VecDeque::new();

        loop {
            should_break = false;
            should_auth = None;

            self.send_frame_req();

            for output in self.state.outputs.values_mut() {
                if should_recreate_surface.pop_front_if(|name| output.name == *name).is_some() {
                    let surface = self.state.wgpu.instance.create_surface(SurfaceTarget::Window(Box::new(
                        output.surface_info.surface_handle,
                    ))).unwrap();
                    output.surface_info.wgpu_surface.init(surface);
                }

                let mut exit: Action = Action::None;
                
                let name = (*output.display_name).clone();
                
                #[cfg(feature = "screenshot")]
                let screen_path = RefCell::new(None);

                let run_ui = coerce_hrtb(|ui| {
                    output_fn(&name, ui, &mut exit);

                    match &exit {
                        Action::None => {},
                        #[cfg(feature = "screenshot")]
                        Action::TakeScreenshot(path_buf) => {
                            *screen_path.borrow_mut() = Some(path_buf.clone())
                        },
                        Action::ForceExit => should_break = true,
                        Action::PasswdCheck(passwd) if !doing_auth => {
                            should_auth = Some(passwd.clone())
                        },
                        Action::PasswdCheck(_) => {}, // dont register if it was already doing auth
                    }
                });

                let context = FrameContext {
                    wgpu: &self.state.wgpu,
                    event_queue: &self.event_queue,
                    new_events: &mut self.state.new_events,
                    egui_renderer: &self.state.egui_renderer,
                    #[cfg(feature = "screenshot")]
                    take_screenshot: &screen_path,
                    #[cfg(feature = "screenshot")]
                    worker: self.state.worker.as_ref().expect("worker should be available here")
                };


                if state::App::frame_to_output(context, output, run_ui).is_err() {
                    should_recreate_surface.push_back(output.name);
                }
            }

            // split to ensure short-circuiting behavior on should_break
            if should_break {
                break;
            } else if doing_auth {
                if let Some(maybe_exit) = self.state.worker.as_mut().expect("worker should be available here").try_recv() {
                    doing_auth = false;
                    if maybe_exit {
                        break;
                    }
                }
            } else if let Some(pwd) = should_auth && !doing_auth {
                doing_auth = true;
                self.state.worker.as_ref().expect("worker should be available here").send(WorkerEvent::CheckPassword(pwd));
            }
        }

        self.event_queue.roundtrip(&mut self.state).unwrap();

        self.state.session_lock.unlock_and_destroy();
        self.event_queue.roundtrip(&mut self.state).unwrap();
        if let Some(w) = self.state.worker.take() {
            w.join();
        }
    }
}

fn coerce_hrtb<F: for<'a> FnMut(&'a mut egui::Ui)>(f: F) -> F { f }

pub mod prelude {
    pub use crate::state::App;
    pub use crate::Action;
    pub use crate::widgets;
}