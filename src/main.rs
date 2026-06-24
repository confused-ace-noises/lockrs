use std::{cell::LazyCell, ffi::{CStr, c_char}, mem, panic::{AssertUnwindSafe, UnwindSafe, catch_unwind}, sync::LazyLock, time::Duration};

use egui::{Color32, ColorImage, Frame, Image, ImageData, Key, RichText, TextureOptions, Vec2, load::SizedTexture};
use image::ImageReader;
use libc::{getpwuid, getpwuid_r, getuid, passwd, ptrace_rseq_configuration};
use wgpu::{CurrentSurfaceTexture, Operations, naga::Statement::Break, wgt::TextureViewDescriptor};
use wl_lock::state::App;

use pam::Client;


fn main() {
    let mut app = App::init();

    app.create_surfaces();
    app.init_wgpu();
    app.init_egui();
    app.init_input();

    let uid = unsafe { getuid() };
    let mut pwd: passwd = unsafe { mem::zeroed() };
    let mut buf = vec![0i8; 1024];
    let mut res = std::ptr::null_mut();

    unsafe {
        getpwuid_r(uid, &mut pwd as *mut passwd, buf.as_mut_ptr() as *mut c_char, buf.len(), &mut res);
    }

    let username = unsafe { CStr::from_ptr(pwd.pw_name) }.to_string_lossy().to_string().clone();

    
    let mut string = String::new();

    //pam_client.conversation_mut().set_credentials(name, password);
    
    app.image_capabilities();
    
    'outer: loop {
        
        let mut should_break = false;
        let mut do_auth = false;
        let names = app.state.outputs.keys().cloned().collect::<Vec<_>>();
        
        app.send_frame_req();
        
        for name in names {
            app.frame_to_output(name, |ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| {
                    Image::new(egui::include_image!("../wallhaven-sails.jpg")).paint_at(ui, ui.ctx().content_rect());
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 2.0 - 20.0); // center
                        
                        ui.add(
                            egui::TextEdit::singleline(&mut string)
                            .desired_width(300.0) 
                            .hint_text("Password...")
                            .password(true),
                        );
                        
                        if ui.input(|input| input.key_pressed(egui::Key::Backspace)) {
                            should_break = true;
                        } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            do_auth = true;
                        }
                    });
                });
            });
            
        }
        if should_break {
            break;
        } else if do_auth {
            let mut pam_client = Client::with_password("login").expect("failed to start PAM client");
            println!("tried: {username}, {string}");
            pam_client.conversation_mut().set_credentials(&username, &string);
            match pam_client.authenticate() {
                Ok(_) => {
                    println!("success!");
                    string.clear();
                    break 'outer;
                },
                Err(e) => {
                    println!("failed: {e:?}");
                    string.clear();
                    continue 'outer;
                } 
            }
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

#[test]
fn test() {
    let uid = unsafe { getuid() };
    let mut pwd: passwd = unsafe { mem::zeroed() };
    let mut buf = vec![0i8; 1024];
    let mut res = std::ptr::null_mut();

    unsafe {
        getpwuid_r(uid, &mut pwd as *mut passwd, buf.as_mut_ptr() as *mut c_char, 1024, &mut res);
    }

    // drop(buf);

    let name = unsafe { CStr::from_ptr(pwd.pw_name) }.to_string_lossy();
    let passwd = unsafe { CStr::from_ptr(pwd.pw_passwd) }.to_string_lossy();

    println!("{uid}: {name}, {passwd}");
}