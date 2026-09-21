//! [`App`]- and [`State`]-related functionality

#[cfg(feature = "screenshot")]
use std::{cell::RefCell, path::PathBuf};
use std::{collections::HashMap, ffi::CStr, mem, os::raw::c_char, rc::Rc, sync::{Arc, Mutex}};

use egui::{Context, Modifiers, Ui};
use egui_wgpu::{Renderer, RendererOptions};
use libc::{getpwuid_r, getuid, passwd};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_callback::WlCallback,
        wl_compositor::WlCompositor,
        wl_display::WlDisplay,
        wl_keyboard::WlKeyboard,
        wl_output::WlOutput,
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::Capability,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1, ext_session_lock_v1::ExtSessionLockV1,
};
#[cfg(feature = "screenshot")]
use wgpu::{Origin3d, TexelCopyTextureInfoBase};
use wgpu::{
    Adapter, BackendOptions, Backends, CompositeAlphaMode, CurrentSurfaceTexture, Device, Instance, InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds, Operations, PowerPreference, PresentMode, Queue, RequestAdapterOptions, SurfaceColorSpace, SurfaceTarget, TextureFormat, TextureUsages, TextureViewDescriptor, wgt::{DeviceDescriptor, SurfaceConfiguration, WgpuHasDisplayHandle},
};
use xkbcommon::xkb::{self};

use crate::{
    Output, Seat, WaylandDisplayH, WaylandSurfaceH, state::worker::LockrsWorker, utils::{global::Global, late::Late},
};

pub mod seat;
pub mod session_lock;
pub mod wl_registry;
pub mod worker;

/// App is the main way that information avaliable through the life of
/// the program is stored.
///
/// # Initializing App manually
/// if you need to initialized App manually (i.e., not calling [`App::init`]),
/// you must set `state.init_done` to true once every [`State`] element has been initialized.
/// See the [`App::_init`] documentation for further information
pub struct App {
    /// connection to the wayland compositor
    pub connection: Connection,
    /// event queue for the wayland client
    pub event_queue: EventQueue<State>,
    /// wayland display
    pub display: WlDisplay,
    /// the [`App`] state, that tracks globals
    /// and other changing app state
    pub state: State,
}

/// The state of [`App`]
pub struct State {
    /// wl_compositor global
    pub compositor: Late<Global<WlCompositor>>,

    /// display handle
    pub display_handle: WaylandDisplayH,

    /// seats stored as (global_name, [`Seat`])
    pub seats: HashMap<u32, Seat>,

    /// input state
    pub input: Late<Input>,

    /// wgpu state
    pub wgpu: Late<WgpuInfo>,

    /// egui renderer
    pub egui_renderer: Late<Mutex<Renderer>>,

    /// ext_session_lock_manager_v1 global
    pub lock_manager: Late<Global<ExtSessionLockManagerV1>>,

    /// ext_session_lock_v1 protocol
    pub session_lock: Late<ExtSessionLockV1>,

    /// seats stored as (global_name, [`Output`])
    pub outputs: HashMap<u32, Output>,

    /// has finished initiatilization. If this is set to true, all
    /// [`Late`]-wrapped globals and states have been initialized and are
    /// now freely dereferenceable. This is only set by [`App::init`], and if you
    /// don't use it (and instead manually initialized the [`State`]), you should set
    /// this to true once you've asserted all [`Late`]s have been initialized.
    pub init_done: bool,

    /// exit with exit code
    pub exit: Option<u32>,

    /// whether the lock screen is currently activated
    pub is_locked: bool,

    /// new events happened; if this is set to true, it'll force a render
    /// on the next frame
    pub new_events: bool,

    /// PAM state
    pub pam: Late<Pam>,

    pub worker: Option<LockrsWorker>
}

impl State {
    pub const DOTS_PER_LINE: f32 = 15.0;
    pub fn new(display_handle: WaylandDisplayH) -> Self {
        Self {
            compositor: Late::uninit(),
            display_handle,
            seats: HashMap::new(),
            input: Late::uninit(),
            wgpu: Late::uninit(),
            egui_renderer: Late::uninit(),
            lock_manager: Late::uninit(),
            session_lock: Late::uninit(),
            outputs: HashMap::new(),
            init_done: false,
            exit: None,
            is_locked: false,
            new_events: false,
            pam: Late::uninit(),
            worker: None,
        }
    }
}

/// PAM data returned by `getpwuid_r` call.
///
/// Initialized by [`App::init_pam`].
#[derive(Debug, Clone)]
pub struct Pam {
    pub uid: u32,
    pub username: Arc<String>,
}

/// Input data.
///
/// Initialized by [`App::init_input`]
pub struct Input {
    pub xkb_ctx: xkb::Context,
    pub pointer: Option<Pointer>,
    pub keyboard: Option<Kb>,
}

pub struct Kb {
    pub focused_output: Option<u32>,
    pub wl_keyboard: WlKeyboard,
    pub xkb_state: Late<xkb::State>,
    pub key_mods: egui::Modifiers,
}

pub struct Pointer {
    pub focused_output: Option<u32>,
    pub wl_pointer: WlPointer,
    pub last_focused_output_in_events: Option<u32>,
    pub last_pointer_pos: Option<(f32, f32)>,
}

/// representation of a pointer event, that splits events in
/// normal events and Axis events, because the latter are usually in logical batches.
pub enum PointerEvent {
    Event(wl_pointer::Event),
    Axis {
        ordered_events: Vec<wl_pointer::Event>,
        source: Option<wl_pointer::AxisSource>,
        /// bitfield:
        /// 0b00000001 -> Axis
        /// 0b00000010 -> AxisValue120
        /// 0b00000100 -> AxisDiscrete
        available_modes: u8,
        is_stop: Option<bool>,
    },
}

pub struct WgpuInfo {
    pub instance: Instance,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
}

pub struct EguiInfo {
    pub context: Context,
    pub renderer: Mutex<Renderer>,
}

impl App {
    /// creates a connection to the compositor and initializes globls
    /// during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    /// </br>
    /// </br>
    /// When manually initializing [`App`], remember to set
    /// `state.init_done` to true once every [`State`] element has been initialized.
    ///
    /// ## Init order
    /// The proper init order is the following:
    /// ```ignore
    /// let mut app = App::_init();
    /// unsafe {
    ///     app.create_surfaces();
    ///     app.init_wgpu();
    ///     app.init_egui();
    ///     app.init_input();
    ///     app.init_pam();
    ///     app.image_capabilities();
    ///     app.init_worker();
    /// }
    /// app.state.init_done = true;
    /// ```
    ///
    /// # Initializes
    /// IF the compositor advertises the proper globals, it will initialize:
    /// - `state.compositor`
    /// - `state.lock_manager`
    /// - `state.outputs`, where for output in `state.outputs`, these are uninitialized:
    ///   `output.egui_context`, `output.surface_info`, `output.display_name`
    ///
    /// otherwise, it will panic.
    pub fn _init() -> App {
        let conn = Connection::connect_to_env().expect("Couldn't connect to wayland server");

        let mut event_queue = conn.new_event_queue::<State>();
        let qh = event_queue.handle();

        let display_handle = WaylandDisplayH::new(&conn);
        let mut state = State::new(display_handle);

        let display = conn.display();
        let _registry = display.get_registry(&qh, ());

        event_queue.roundtrip(&mut state).unwrap(); // globals

        assert!(
            state.compositor.is_init() && state.lock_manager.is_init(),
            "globals failed to init. The compositor doesn't support the needed globals. If it does, report this."
        );

        if state.outputs.is_empty() {
            eprintln!("WARNING: no outputs were advertised by the compositor")
        }

        App {
            connection: conn,
            event_queue,
            state,
            display,
        }
    }

    /// Initializes [`App`].
    ///
    /// If you wish to manually initialize [`App`], refer to [`App::_init`]
    pub fn init() -> App {
        let mut app = App::_init();

        unsafe {
            app.create_surfaces();
            app.init_wgpu();
            app.init_egui();
            app.init_input();
            app.init_pam();
            app.init_image_capabilities();
            app.init_worker();
        }

        app.state.init_done = true;

        app
    }

    /// creates surfaces during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - `state.session_lock`
    /// - for output in `state.outputs`, `output.surface_info`, where:
    ///   - `output.surface_info.width` and `output.surface_info.height` are initialized via a compositor configure event;
    ///   - `output.surface_info.wgpu_surface` is uninitialized.
    ///
    /// # Safety
    /// This assumes that the `state.compositor` and `state.lock_manager` [`Late`] globals have
    /// been initialized. It is UB to call this function without asserting that they are.
    /// This also assumes that `state.outputs` has been populated.
    pub unsafe fn create_surfaces(&mut self) {
        let qh = self.event_queue.handle();

        let compositor = &self.state.compositor;
        let session_lock_manager = &self.state.lock_manager;
        let lock = session_lock_manager.lock(&qh, ());

        for (name, output) in &mut self.state.outputs {
            let wl_surface = compositor.create_surface(&qh, *name);
            let role = lock.get_lock_surface(&wl_surface, &output.wl_output, &qh, *name);

            let handle = WaylandSurfaceH::new(&wl_surface);

            output.surface_info.init(crate::SurfaceInfo {
                surface: wl_surface,
                lock_surface: role,
                surface_handle: handle,
                width: Late::uninit(),
                height: Late::uninit(),
                pending_scaling: Late::uninit(),
                wgpu_surface: Late::uninit(),
            });
        }

        self.state.session_lock.init(lock);

        // get configure events
        self.event_queue.roundtrip(&mut self.state).unwrap();
    }

    /// initializes input and seat data during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - `state.input`
    /// - for `seat` in `state.seats`, if `seat.capabilities` is [`Option::Some`], any of:
    ///     - `state.input.keyboard`, where `state.input.keyboard.xkb_state` is uninitialized;
    ///     - `state.input.pointer`
    ///
    /// # Safety
    /// This assumes that the for `output` in `state.outputs`, `output.surface_info` has
    /// been initialized. It is UB to call this function without asserting that it is.
    pub unsafe fn init_input(&mut self) {
        self.state.input.init(Input {
            xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            pointer: None,
            keyboard: None,
        });

        let qh = self.event_queue.handle();

        for seat in self.state.seats.values() {
            if let Some(caps) = seat.capabilities {
                match caps {
                    wayland_client::WEnum::Value(cap) => {
                        if cap.contains(Capability::Keyboard) {
                            let wl_keyboard = seat.wl_seat.get_keyboard(&qh, ());
                            self.state.input.keyboard = Some(Kb {
                                focused_output: None,
                                wl_keyboard,
                                xkb_state: Late::uninit(),
                                key_mods: Modifiers::NONE,
                            })
                        }

                        if cap.contains(Capability::Pointer) {
                            let wl_pointer = seat.wl_seat.get_pointer(&qh, ());
                            self.state.input.pointer = Some(Pointer {
                                focused_output: None,
                                wl_pointer,
                                last_focused_output_in_events: None,
                                last_pointer_pos: None,
                            })
                        }
                    }
                    wayland_client::WEnum::Unknown(_) => unimplemented!(),
                }
            }
        }

        self.event_queue.roundtrip(&mut self.state).unwrap();
    }

    /// initializes wgpu data during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - state.wgpu
    /// - for `output` in `state.outputs`, `output.surface_info.wgpu_surface`
    ///
    /// # Safety
    /// This assumes that for `output` in `state.outputs`, `output.surface_info` has
    /// been initialized. It is UB to call this function without asserting that it is.
    pub unsafe fn init_wgpu(&mut self) {
        let instance = wgpu::Instance::new(Self::wgpu_instance_desc(self.state.display_handle));

        for output in self.state.outputs.values_mut() {
            let wgpu_surface = instance
                .create_surface(SurfaceTarget::Window(Box::new(
                    output.surface_info.surface_handle,
                )))
                .unwrap();

            output.surface_info.wgpu_surface.init(wgpu_surface);
        }

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(
                &self
                    .state
                    .outputs
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .surface_info
                    .wgpu_surface,
            ),
            ..Default::default()
        });

        let adapter = pollster::block_on(adapter).unwrap();

        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).unwrap();

        self.state.outputs.iter_mut().for_each(|(_, output)| {
            let ppp = if output.surface_info.pending_scaling.is_init() { *output.surface_info.pending_scaling } else { 1. };
            output.surface_info.wgpu_surface.configure(
                &device,
                &Self::wgpu_surface_config((*output.surface_info.width as f32 * ppp).round() as u32, (*output.surface_info.height as f32 * ppp).round() as u32),
            );
        });

        self.state.wgpu.init(WgpuInfo {
            instance,
            adapter,
            device,
            queue,
        });
    }

    fn wgpu_surface_config(width: u32, height: u32) -> SurfaceConfiguration<Vec<TextureFormat>> {
        SurfaceConfiguration {
            #[cfg(feature = "screenshot")]
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            #[cfg(not(feature = "screenshot"))]
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width,
            height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
            color_space: SurfaceColorSpace::Auto,
        }
    }

    fn wgpu_instance_desc(display: impl WgpuHasDisplayHandle) -> InstanceDescriptor {
        InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
            display: Some(Box::new(display)),
        }
    }

    /// initializes egui data during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - `state.egui_renderer`
    /// - for `output` in `state.outputs`, `output.egui_context`
    ///
    /// # Safety
    /// This assumes that state.wgpu has been initialized. It is UB to call
    /// this function without asserting that it is.
    /// It also assumes that `state.outputs` ahs already been populated.
    pub unsafe fn init_egui(&mut self) {
        for output in self.state.outputs.values_mut() {
            let ctx = Context::default();
            if output.surface_info.pending_scaling.is_init() {
                ctx.set_pixels_per_point(*output.surface_info.pending_scaling);
            }
            ctx.input_mut(|x| x.max_texture_side = 8000);
            output.egui_context.init(ctx);
        }

        let renderer = egui_wgpu::Renderer::new(
            &self.state.wgpu.device,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            RendererOptions::default(),
        );

        self.state.egui_renderer.init(Mutex::new(renderer));
    }

    /// initializes the egui image loaders during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Safety
    /// This assumes that for `output` in `state.outputs`, `output.egui_context` has
    /// been initialized. It is UB to call this function without asserting that it is.
    /// It also assumes that `state.outputs` ahs already been populated.
    pub unsafe fn init_image_capabilities(&mut self) {
        for output in self.state.outputs.values() {
            egui_extras::install_image_loaders(&output.egui_context)
        }
        self.event_queue.roundtrip(&mut self.state).unwrap();
    }

    /// initializes PAM data during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - `state.pam`
    pub fn init_pam(&mut self) {
        let uid = unsafe { getuid() };
        let mut pwd: passwd = unsafe { mem::zeroed() };
        let mut buf = vec![0i8; 1024];
        let mut res = std::ptr::null_mut();

        unsafe {
            getpwuid_r(
                uid,
                &mut pwd as *mut passwd,
                buf.as_mut_ptr() as *mut c_char,
                buf.len(),
                &mut res,
            );
        }

        let username = unsafe { CStr::from_ptr(pwd.pw_name) }
            .to_string_lossy()
            .to_string()
            .clone();

        self.state.pam.init(Pam {
            uid,
            username: Arc::new(username),
        });
    }

    /// initializes the lockrs worker thread during [`App`] init. This is normally called by [`App::init`],
    /// but may be called manually if initializing [`App`] manually.
    ///
    /// # Initializes
    /// - `state.worker`
    /// 
    /// # Safety
    /// This assumes that `state.pam` has been initialized. 
    /// It is UB to call this function without asserting that it is.
    pub unsafe fn init_worker(&mut self) {
        let pam = &*self.state.pam;

        let worker = LockrsWorker::new(pam.clone());

        self.state.worker = Some(worker);
    }

    pub fn pam_auth(pam: &Pam, passwd: &String) -> bool {
        let mut pam_client =
            pam::Client::with_password("login").expect("failed to start PAM client");
        pam_client
            .conversation_mut()
            .set_credentials(pam.username.as_ref(), passwd);
        pam_client.authenticate().is_ok()
    }

    /// send a frame request to the compositor for each output.
    pub fn send_frame_req(&mut self) {
        let qh = self.event_queue.handle();

        for output in self.state.outputs.values() {
            output.surface_info.surface.frame(&qh, ());
        }
        self.event_queue.roundtrip(&mut self.state).unwrap();
    }

    /// render a frame to a specific output.
    pub fn frame_to_output<'a>(
        frame_ctx: FrameContext<'a>,
        output: &'a mut Output,
        run_ui: impl for<'b> FnMut(&'b mut Ui),
    ) -> Result<(), SurfaceLost> {
        let device = &frame_ctx.wgpu.device;
        // let output = frame_ctx.output;
        let wgpu_surface = &output.surface_info.wgpu_surface;
        let wgpu_queue = &frame_ctx.wgpu.queue;
        let ctx = &output.egui_context;

        let width = *output.surface_info.width;
        let height = *output.surface_info.height;

        if !(*frame_ctx.new_events || output.egui_context.has_requested_repaint()) {
            return Ok(());
        }

        let ppp = ctx.pixels_per_point();
        *frame_ctx.new_events = false;

        let surface_texture = loop {
            match wgpu_surface.get_current_texture() {
                CurrentSurfaceTexture::Success(texture) => break texture,
                CurrentSurfaceTexture::Suboptimal(texture) => {
                    drop(texture);
                    wgpu_surface.configure(
                        device, 
                        &Self::wgpu_surface_config((width as f32 * ppp).round() as u32, (height as f32 * ppp).round() as u32)
                    );
                },
                CurrentSurfaceTexture::Outdated => {
                    wgpu_surface.configure(
                        device, 
                        &Self::wgpu_surface_config((width as f32 * ppp).round() as u32, (height as f32 * ppp).round() as u32)
                    );
                },
                CurrentSurfaceTexture::Timeout 
                    | CurrentSurfaceTexture::Occluded
                    | CurrentSurfaceTexture::Validation => return Ok(()),
                CurrentSurfaceTexture::Lost => return Err(SurfaceLost),
            };
        };

        let mut encoder = device.create_command_encoder(&Default::default());

        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());


        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [(width as f32 * ppp).round() as u32, (height as f32 * ppp).round() as u32],
            pixels_per_point: ppp,
        };

        let raw_input: egui::RawInput = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(width as f32, height as f32),
            )),
            events: mem::take(&mut output.events_to_flush),
            ..Default::default()
        };

        let mut full_output = ctx.run_ui(raw_input, run_ui);

        let primitives = ctx.tessellate(full_output.shapes, ctx.pixels_per_point());

        let mut renderer = frame_ctx.egui_renderer.lock().unwrap();

        for (id, delta) in full_output.textures_delta.set.drain() {
            for d in &delta {
                renderer.update_texture(device, wgpu_queue, id, d);
            }
        }

        renderer.update_buffers(
            device,
            wgpu_queue,
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

        #[cfg(feature = "screenshot")]
        if let Some(ref path) = *frame_ctx.take_screenshot.borrow() {
            use crate::state::worker::SaveImageData;

            let physical_h = (height as f32 * ppp).round() as u32;
            let physical_w = (width as f32 * ppp).round() as u32;
            let padded_bytes_per_row =
                wgpu::util::align_to(physical_w * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    
            let buffer_size = (padded_bytes_per_row * physical_h) as u64;
    
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("screenshot staging buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
    
            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfoBase {
                    texture: &surface_texture.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                }, 
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(physical_h),
                    },
                },
                wgpu::Extent3d {
                    width: physical_w,
                    height: physical_h,
                    depth_or_array_layers: 1,
                },
            );
    
            wgpu_queue.submit([encoder.finish()]);
    
            let slice = buffer.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).unwrap();
    
            let data = slice.get_mapped_range().unwrap();
    
            frame_ctx.worker.send(worker::WorkerEvent::SaveImage(SaveImageData {
                buffer,
                data,
                physical_h,
                physical_w,
                padded_bytes_per_row,
                path: path.clone(),
            }));
        } else {
            wgpu_queue.submit([encoder.finish()]);
        }
        #[cfg(not(feature = "screenshot"))]
        wgpu_queue.submit([encoder.finish()]);

        wgpu_queue.present(surface_texture);
        frame_ctx.event_queue.flush().unwrap();
        Ok(())
    }
}

pub struct SurfaceLost;

pub struct FrameContext<'a> {
    pub wgpu: &'a WgpuInfo,
    pub event_queue: &'a EventQueue<State>,
    pub new_events: &'a mut bool,
    pub egui_renderer: &'a Mutex<Renderer>,
    #[cfg(feature = "screenshot")]
    pub take_screenshot: &'a RefCell<Option<PathBuf>>,
    #[cfg(feature = "screenshot")]
    pub worker: &'a LockrsWorker
}

impl State {
    pub const MIN_WL_COMPOSITOR_VER: u32 = 6;
    pub const MIN_WL_SEAT_VER: u32 = 9;
    pub const MIN_WL_SUBCOMPOSITOR_VER: u32 = 1;
    pub const MIN_ZWLR_LAYER_SHELL_VER: u32 = 4;

    pub fn bind<T>(
        bind_to: &mut Late<Global<T>>,
        proxy: &WlRegistry,
        name: u32,
        qh: &QueueHandle<Self>,
        version: u32,
    ) where
        T: Proxy + 'static,
        Self: Dispatch<T, ()>,
    {
        bind_to.init(Global::new(proxy.bind(name, version, qh, ()), name));
    }
}

delegate_noop!(State: WlCompositor);
delegate_noop!(State: ExtSessionLockManagerV1);

// name of the output this surface belongs to
//                       vvv
impl Dispatch<WlSurface, u32> for State {
    fn event(
        state: &mut Self,
        proxy: &WlSurface,
        event: <WlSurface as Proxy>::Event,
        output_name: &u32,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_surface::Event::Enter { .. } => {},
            wayland_client::protocol::wl_surface::Event::Leave { .. } => {},
            wayland_client::protocol::wl_surface::Event::PreferredBufferScale { factor } => {
                if let Some(o) = state.outputs.get_mut(output_name) {
                    o.surface_info.pending_scaling.init(factor as f32)
                }
                proxy.set_buffer_scale(factor);
            },
            wayland_client::protocol::wl_surface::Event::PreferredBufferTransform { .. } => {},
            _ => unimplemented!(),
        }
    }
}

impl Dispatch<WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        data: &u32,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_output::Event::Name { name } = event {
            let output = state.outputs.get_mut(data).unwrap();
            output.display_name.init(Rc::new(name));
        }
    }
}

delegate_noop!(State: ignore WlBuffer);

impl Dispatch<WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_callback::Event::Done { .. } = event {
            state.new_events = true;
        }
    }
}
