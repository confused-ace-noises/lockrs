
use wl_lock::state::App;

fn main() {
    let app = App::init();

    let qh = app.event_queue.handle();

    let session_lock = app.state.lock_manager.global.lock(&qh, ());

    
    for (name, output) in app.state.outputs {
        let wl_surface = app.state.compositor.global.create_surface(&qh, ());
        let _lock_surface = session_lock.get_lock_surface(&wl_surface, &output.wl_output, &qh, name);
        
    }

}
