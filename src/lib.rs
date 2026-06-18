use wayland_client::{
    WEnum, protocol::{
        wl_output::WlOutput, wl_seat::{Capability, WlSeat}, wl_surface::WlSurface
    }
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;

use crate::utils::late::Late;

pub mod utils;
pub mod state;

pub struct Seat {
    pub wl_seat: WlSeat,
    pub capabilities: Option<WEnum<Capability>>,
    pub name: Option<String>,
}

pub struct Output {
    pub wl_output: WlOutput,
    pub surface: Late<WlSurface>,
    pub lock_surface: Late<ExtSessionLockSurfaceV1>,
    pub width: u32, 
    pub height: u32,
    pub name: u32,
    pub configured: bool,
}

impl Output {
    pub fn new_uninit(wl_output: WlOutput, name: u32) -> Self {
        Self {
            wl_output,
            surface: Late::uninit(),
            lock_surface: Late::uninit(),
            width: 0,
            height: 0,
            name,
            configured: false,
        }
    }
}



// impl Dispatch<WlOutput, ()> for State {
//     fn event(
//         state: &mut Self,
//         proxy: &WlOutput,
//         event: <WlOutput as Proxy>::Event,
//         data: &(),
//         conn: &Connection,
//         qhandle: &QueueHandle<Self>,
//     ) {
//         match event {
//             wl_output::Event::Geometry { x, y, physical_width, physical_height, subpixel, make, model, transform } => todo!(),
//             wl_output::Event::Mode { flags, width, height, refresh } => todo!(),
//             wl_output::Event::Done => todo!(),
//             wl_output::Event::Scale { factor } => todo!(),
//             wl_output::Event::Name { name } => todo!(),
//             wl_output::Event::Description { description } => todo!(),
//             _ => todo!(),
//         }
//     }
// }