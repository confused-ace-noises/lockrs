use std::{path::PathBuf, sync::mpsc::{self, SendError, Sender, channel}, thread::{self, JoinHandle}};

use wgpu::{Buffer, BufferView};

use crate::state::{App, Pam};

/// Data passed to a [`LockrsWorker`] in a [`WorkerEvent`]
/// that carries the data needed to take a screenshot and 
/// clean up after.
#[cfg(feature = "screenshot")]
pub struct SaveImageData {
    pub buffer: Buffer, 
    pub data: BufferView,
    pub physical_h: u32,
    pub physical_w: u32,
    pub padded_bytes_per_row: u32,
    pub path: PathBuf,
}

/// An event passed to [`LockrsWorker`]
pub enum WorkerEvent {
    /// Stops the worker thread
    Stop,
    /// takes a screenshot
    #[cfg(feature = "screenshot")]
    SaveImage(SaveImageData),
    /// makes a password check
    CheckPassword(String),
}

/// A worker thread that handles slow operations, such as
/// checking the password and saving a screenshot, to that
/// the main UI thread doesn't get stopped
pub struct LockrsWorker {
    sender: mpsc::Sender<WorkerEvent>,
    handle: JoinHandle<()>,
    passwd_response: mpsc::Receiver<bool>,
    backup_password_sender: mpsc::Sender<bool>,
    pam: Pam
}

impl LockrsWorker {

    /// Sends a stop [`WorkerEvent`] and joins the worker thread
    pub fn join(self) {
        let _ = self.sender.send(WorkerEvent::Stop);
        let _ = self.handle.join();
    }

    /// Checks the password result checking channel for results.
    /// If there's no result at the time of check, it'll return `None`, otherwise
    /// it'll return `Some(true)` or `Some(false)` if the password check was 
    /// successful or it failed, respectively.
    pub fn try_recv(&mut self) -> Option<bool> {
        self.passwd_response.try_recv().ok()
    }

    /// Send a new [`WorkerEvent`] to the worker thread.
    /// If the worker thread died for some reason, it'll fall back 
    /// to a sync execution, stopping the calling thread.
    pub fn send(&self, event: WorkerEvent) {
        if let Err(SendError(ev)) = self.sender.send(event) {
            eprintln!("falling back to sync execution, worker thread is dead");

            match ev {
                WorkerEvent::Stop => {},
                #[cfg(feature = "screenshot")]
                WorkerEvent::SaveImage(save_image_data) => Self::handle_save_image(save_image_data),
                WorkerEvent::CheckPassword(passwd) => {
                    let _ = Self::handle_password_check(&self.pam, &passwd, &self.backup_password_sender);
                },
            }
        }
    } 


    /// does the screenshot taking stuff
    #[cfg(feature = "screenshot")]
    fn handle_save_image(SaveImageData { buffer, data, physical_h, physical_w, padded_bytes_per_row, path }: SaveImageData) {
        let mut rgba = Vec::with_capacity((physical_w * physical_h * 4) as usize);

        for row in 0..physical_h as usize {
            let start = row * padded_bytes_per_row as usize;
            let end = start + (physical_w * 4) as usize;
            rgba.extend(data[start..end].as_chunks::<4>().0.iter().flat_map(|[b, g, r, a]| [r, g, b, a]));
        }

        drop(data);
        buffer.unmap();
        
        let _ = image::save_buffer(
            path,
            &rgba,
            physical_w,
            physical_h,
            image::ColorType::Rgba8,
        );
    }

    /// does the password check
    fn handle_password_check(pam: &Pam, passwd: &String, sender: &Sender<bool>) -> bool {
        let res = App::pam_auth(pam, passwd);
        sender.send(res).is_ok()
    }

    /// Create a new LockrsWorker; this launches the helper thread.
    pub fn new(pam: Pam) -> Self {
        let (tx, rx) = channel::<WorkerEvent>();

        let (inner_tx, inner_rx) = channel::<bool>();

        let backup_sender = inner_tx.clone();

        let backup_pam = pam.clone();

        let handle = thread::spawn(move || {
            'main_loop: while let Ok(event) = rx.recv() {
                match event {
                    WorkerEvent::Stop => break 'main_loop,

                    #[cfg(feature = "screenshot")]
                    WorkerEvent::SaveImage(data) => Self::handle_save_image(data),

                    WorkerEvent::CheckPassword(passwd) => {
                        if !Self::handle_password_check(&pam, &passwd, &inner_tx) {
                            break 'main_loop;
                        }
                    }
                }
            }
            
        });

        Self {
            sender: tx,
            handle,
            passwd_response: inner_rx,
            backup_password_sender: backup_sender,
            pam: backup_pam,
        }
    }
}