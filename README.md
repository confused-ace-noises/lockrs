# Lockrs
![MIT or Apache 2.0 Licensed](https://img.shields.io/badge/license-MIT_OR_Apache%202.0-blue?style=for-the-badge)

Lockrs is a library that allows egui to render to a lock screen through the `ext-session-lock-v1` protocol. It also allows for different output on different monitors.

## Why?
Lockrs aims to be a scaffold for screen lockers, NOT a screen locker in of itself. You may also need to get into the fields of the library's types, if you need to do fancy stuff. 
So, the whole reason for this project's existence isn't to provide a ready-to-use lockscreen, but to be a way to make the most customizable lockscreen possible without going to down to protocol primitives.

## How?

To use the library, you first need to initialize the `App` struct through the `init` method. This will connect to the compositor, handle the surfaces, initialize `wgpu` and `egui` and handle the low-level stuff, in general. Once you init the `App` struct, you can use the `ui` method to talk to `egui`.

Note: it's heavily recommended to compile this crate with `opt-level = 3`, because the image loading that egui does for the background, for example, is ***significanlty*** sped up (a few seconds to a few milliseconds). This may achieved by manually setting the `opt-level` for the desidered profile in the `Cargo.toml`, or compiling with the `--release` flag.

## Example
This is my personal lock screen:
```rs
// ---- deps ----
use egui::{Color32, Image, include_image};
use lockrs::prelude::*;
// ---- deps ----

// main
let mut app = App::init();

let mut password = String::new();

app.ui(|_output_name, ui, exit| {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            Image::new(include_image!("path/to/background")).paint_at(ui, ui.ctx().content_rect());

            center_vertical(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add(
                        widgets::Clock::new()
                            .time_style(|rich_text| rich_text.size(81.0).color(Color32::BLACK))
                            .date_style(|rich_text| rich_text.size(27.0).color(Color32::BLACK)),
                    );

                    ui.add(
                        egui::TextEdit::singleline(&mut password)
                            .desired_width(300.0)
                            .hint_text("Password...")
                            .horizontal_align(egui::Align::Center)
                            .password(true),
                    );

                    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                        *exit = TryExit::Force
                    } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        *exit = TryExit::PasswdCheck(password.clone())
                    }
                })
            })
        });
});
```

this code will produce the following output:
![screenshot of lockrs' output](.assets/screenshot.png)

## Usage
To use this library, simply add it in the dependencies of your Rust project:
```toml
# Cargo.toml

[dependencies]
lockrs = "0.3.0" # put latest version here

egui = "0.36.1"
```

### Updates
This library may be updated in the future, so if it does happen, the API will probably change a bit until it's in a more stable situation.

#### License
<small>
Licensed under either of MIT or Apache License, Version 2.0 license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. 
</small>