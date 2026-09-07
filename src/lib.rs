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
//!                         *exit = TryExit::Force
//!                     } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
//!                         *exit = TryExit::PasswdCheck(password.clone())
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

use std::{ffi::c_void, mem, ptr::NonNull};

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
use wgpu::{CurrentSurfaceTexture, Operations, Surface as WgpuSurface, TextureViewDescriptor};
use crate::{state::{App, PointerEvent}, utils::late::Late};

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
    pub display_name: Late<String>,
    /// whether the ext_session_lock_surface_v1 surface has gotten a
    /// configure event
    pub configured: bool,
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


/// describes how to try to exit the lockscreen:
/// - [`TryExit::None`]: don't try to exit the lockscreen.
/// - [`TryExit::Force`]: force the lockscreen to exit without doing a password check.
/// - [`TryExit::PasswdCheck`]: pass a [`String`]; if it matches the password,
///   the lockscreen will exit, otherwise assume that the password check failed. 
pub enum TryExit {
    None,
    Force, 
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
    /// 3. a mutable reference to a [`TryExit`]. This is by defualt [`TryExit::None`]. 
    ///    it to control how the lock screen should exit (note: this pointer points to a different
    ///    TryExit for each output. If different [`TryExit::PasswdCheck`] are set for 
    ///    different output passes, only the last one will be considered.)
    pub fn ui<F: for<'a> FnMut(&String, &'a mut Ui, &mut TryExit)>(&mut self, mut output_fn: F) {
        let mut should_break: bool;
        let mut should_auth: Option<String>;

        loop {
            should_break = false;
            should_auth = None;

            self.send_frame_req();

            for output in self.state.outputs.values_mut() {
                let mut exit: TryExit = TryExit::None;

                let display_name = &*output.display_name;
                
                let run_ui = coerce_hrtb(|ui| {
                    output_fn(display_name, ui, &mut exit);
                });

                {
                    let device = &self.state.wgpu.device;
                    // let output = output;
                    let wgpu_surface = &output.surface_info.wgpu_surface;
                    let ctx = &output.egui_context;

                    let qh = &self.event_queue.handle();

                    output.surface_info.surface.frame(qh, ());

                    let width = *output.surface_info.width;
                    let height = *output.surface_info.height;

                    if !(self.state.new_events || output.egui_context.has_requested_repaint()) {
                        continue;
                    }

                    let surface_texture = match wgpu_surface.get_current_texture() {
                        CurrentSurfaceTexture::Success(texture) => texture,
                        CurrentSurfaceTexture::Suboptimal(texture) => {
                            // wgpu_surface.configure(&self.state.wgpu.device, &Self::wgpu_surface_config(width, height));
                            texture
                        }
                        _ => continue,
                    };

                    let mut encoder = device.create_command_encoder(&Default::default());

                    let view = surface_texture
                        .texture
                        .create_view(&TextureViewDescriptor::default());

                    let screen_descriptor = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [width, height],
                        pixels_per_point: ctx.pixels_per_point(),
                    };

                    let raw_input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::Vec2::new(width as f32, height as f32),
                        )),
                        events: mem::take(&mut output.events_to_flush),
                        ..Default::default()
                    };

                    let mut full_output = ctx.run_ui(raw_input, run_ui);

                    let primitives = ctx.tessellate(full_output.shapes, ctx.pixels_per_point());

                    let mut renderer = self.state.egui_renderer.lock().unwrap();

                    for (id, delta) in full_output.textures_delta.set.drain() {
                        for d in &delta {
                            renderer.update_texture(device, &self.state.wgpu.queue, id, d);
                        }
                    }

                    renderer.update_buffers(
                        device,
                        &self.state.wgpu.queue,
                        &mut encoder,
                        &primitives,
                        &screen_descriptor,
                    );

                    let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: Operations::default(),
                        })],
                        ..Default::default()
                    });

                    let mut pass = pass.forget_lifetime();
                    renderer.render(&mut pass, &primitives, &screen_descriptor);

                    for id in &full_output.textures_delta.free {
                        renderer.free_texture(id);
                    }

                    drop(renderer);
                    drop(pass);

                    self.state.wgpu.queue.submit([encoder.finish()]);
                    self.state.wgpu.queue.present(surface_texture);
                    self.event_queue.flush().unwrap();
                }

                // drop((output, name));

                match exit {
                    TryExit::None => {},
                    TryExit::Force => should_break = true,
                    TryExit::PasswdCheck(pwd) => {
                        should_auth = Some(pwd);
                    },
                }
            }

            // split to ensure short-circuiting behavior on should_break
            if should_break {
                break;
            } else if let Some(pwd) = should_auth && self.pam_auth(&pwd) {
                break;
            }
        }

        self.event_queue.roundtrip(&mut self.state).unwrap();

        self.state.session_lock.unlock_and_destroy();
        self.event_queue.roundtrip(&mut self.state).unwrap();
    }
}

fn coerce_hrtb<F: for<'a> FnMut(&'a mut egui::Ui)>(f: F) -> F { f }

pub mod prelude {
    pub use crate::state::App;
    pub use crate::TryExit;
    pub use crate::widgets;
}