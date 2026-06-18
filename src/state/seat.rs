use wayland_client::{Connection, Dispatch, Proxy, protocol::wl_seat::{self, WlSeat}};

use crate::state::State;

impl Dispatch<WlSeat, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        data: &u32,
        _: &Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        let seat = state
            .seats
            .get_mut(data)
            .expect("Server sent a wl_seat event before registering said seat.");

        match event {
            wl_seat::Event::Capabilities { capabilities } => seat.capabilities = Some(capabilities),
            wl_seat::Event::Name { name } => seat.name = Some(name),
            _ => {}
        }
    }
}
