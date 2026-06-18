
use core::slice;
use std::{os::fd::{AsFd, AsRawFd}, time::Duration};

use libc::{mmap, sleep};
use rustix::fs::{MemfdFlags, ftruncate};
use wayland_client::{Dispatch, QueueHandle, protocol::{wl_buffer::{self, WlBuffer}, wl_shm::{self, WlShm}, wl_shm_pool::{self, WlShmPool}, wl_surface::WlSurface}};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use wl_lock::state::App;

pub struct Surface {
    pub wl_surface: WlSurface,
    pub lock_surface: ExtSessionLockSurfaceV1,
    pub name: u32,
}
pub struct Surface2 {
    surface: Surface,
    pool: WlShmPool,
    buffer: WlBuffer,
    chunked_thing: &'static mut [[u8; 4]],
}

fn main() {
    let mut app = App::init();

    let qh = app.event_queue.handle();

    let session_lock = app.state.lock_manager.global.lock(&qh, ());

    let surfaces =  app.state.outputs.iter().map(|(name, out)| {
        let wl_surface = app.state.compositor.global.create_surface(&qh, ());
        let lock_surface = session_lock.get_lock_surface(&wl_surface, &out.wl_output, &qh, *name);

        // wl_surface.commit();

        Surface {
            wl_surface,
            lock_surface,
            name: *name,
        }
    }).collect::<Vec<_>>();

    app.event_queue.roundtrip(&mut app.state).unwrap(); // get configure for each surface

    let surfaces = surfaces.into_iter().map(|surface | {
        let Surface { wl_surface, lock_surface, name } = &surface;

        let output = app.state.outputs.get_mut(name).expect("Output changed name somehow?");

        let (ptr, pool, buffer) = make_buffer("test-buffer", &qh, &app.state.shm, output.height, output.width);   
        let slice = unsafe { slice::from_raw_parts_mut(ptr, (output.height * output.width * 4) as usize) };
        
        let (chunked, _) = slice.as_chunks_mut::<4>();

        wl_surface.attach(Some(&buffer), 0, 0); // TODO: is this output-relative or is it absolute?

        // mauve
        chunked.iter_mut().for_each(|[b, g, r, a]| {
            *b = 255;
            *g = 176;
            *r = 224;
            *a = 255;
        });

        surface.wl_surface.damage(0, 0, output.width as i32, output.height as i32);

        wl_surface.commit();

        Surface2 {
            buffer,
            surface,
            chunked_thing: chunked,
            pool,
        }
    }).collect::<Vec<_>>();

    app.event_queue.roundtrip(&mut app.state).unwrap();

    std::thread::sleep(Duration::from_secs(2));
    session_lock.unlock_and_destroy();
    app.event_queue.roundtrip(&mut app.state).unwrap();
}

fn make_buffer<S>(
    name: &str,
    qh: &QueueHandle<S>,
    shm: &WlShm,
    height: u32,
    width: u32,
) -> (*mut u8, WlShmPool, WlBuffer) 
where
    S: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
{
    let stride = width * 4;
    let size = stride * height;

    unsafe {
        let fd = rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC).unwrap();

        ftruncate(&fd, size.into()).unwrap();

        let map = mmap(
            std::ptr::null_mut(),
            size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        ) as *mut u8;

        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());

        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        (map, pool, buffer)
    }
}
