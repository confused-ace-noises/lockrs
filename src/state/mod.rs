use std::collections::HashMap;

use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop, protocol::{wl_compositor::WlCompositor, wl_output::WlOutput, wl_registry::WlRegistry, wl_surface::WlSurface}};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1::ExtSessionLockManagerV1;

use crate::{Output, Seat, utils::{global::Global, late::Late}};

pub mod wl_registry;
pub mod session_lock;
pub mod seat;

pub struct App {
    pub connection: Connection,
    pub event_queue: EventQueue<State>,
    pub state: State,
}

#[derive(Default)]
pub struct State {
    pub compositor: Late<Global<WlCompositor>>,
    pub lock_manager: Late<Global<ExtSessionLockManagerV1>>,
    pub seats: HashMap<u32, Seat>,
    
    pub outputs: HashMap<u32, Output>,
    pub init_done: bool,
    pub exit: Option<u32>,

    pub is_locked: bool,
}

impl App {
    pub fn init() -> App {
        let conn = Connection::connect_to_env().expect("Couldn't connect to wayland server");

        let mut event_queue = conn.new_event_queue::<State>();
        let qh = event_queue.handle();

        let mut state = State::default();

        let display = conn.display();
        let _registry = display.get_registry(&qh, ());

        event_queue.roundtrip(&mut state).unwrap(); // globals

        assert!(state.compositor.is_init());

        state.init_done = true;

        App { connection: conn, event_queue, state }
    }    
}


impl State {
    pub const MIN_WL_COMPOSITOR_VER: u32 = 6;
    pub const MIN_WL_SEAT_VER: u32 = 9;
    pub const MIN_WL_SUBCOMPOSITOR_VER: u32 = 1;
    pub const MIN_ZWLR_LAYER_SHELL_VER: u32 = 4;
    pub const MIN_WL_SHM_VER: u32 = 2;

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

delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlOutput);
