use std::time::Duration;

use egui::{Color32, Frame, Key, RichText};
use wgpu::{CurrentSurfaceTexture, Operations, naga::Statement::Break, wgt::TextureViewDescriptor};
use wl_lock::state::App;

fn main() {
    let mut app = App::init();

    app.create_surfaces();
    app.init_wgpu();
    app.init_egui();
    app.init_input();

    let mut string = String::new();

    loop {
        let mut should_break = false;
        let names = app.state.outputs.keys().cloned().collect::<Vec<_>>();

        for name in names {
            app.frame_to_output(name, |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 2.0 - 20.0); // center
    
                        ui.add(
                            egui::TextEdit::singleline(&mut string)
                                .desired_width(300.0) 
                                .hint_text("Password..."),
                        );
    
                        if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            should_break = true;
                        }
                    });
                });
            });
    
        }
        if should_break {
            break;
        }
    }

    println!("input: -- {string} --");

    app.event_queue.roundtrip(&mut app.state).unwrap();

    app.state.session_lock.unlock_and_destroy();
    app.event_queue.roundtrip(&mut app.state).unwrap();

    // std::thread::sleep(Duration::from_secs(2));
}

// let surfaces = surfaces.into_iter().map(|surface | {
//     let Surface { wl_surface, name , ..} = &surface;

//     let output = app.state.outputs.get_mut(name).expect("Output changed name somehow?");

//     let (ptr, pool, buffer) = make_buffer("test-buffer", &qh, &app.state.shm, output.height, output.width);
//     let slice = unsafe { slice::from_raw_parts_mut(ptr, (output.height * output.width * 4) as usize) };

//     let (chunked, _) = slice.as_chunks_mut::<4>();

//     wl_surface.attach(Some(&buffer), 0, 0); // TODO: is this output-relative or is it absolute?

//     // mauve
//     chunked.iter_mut().for_each(|[b, g, r, a]| {
//         *b = 255;
//         *g = 176;
//         *r = 224;
//         *a = 255;
//     });

//     surface.wl_surface.damage(0, 0, output.width as i32, output.height as i32);

//     wl_surface.commit();

//     Surface2 {
//         buffer,
//         surface,
//         chunked_thing: chunked,
//         pool,
//     }
// }).collect::<Vec<_>>();
