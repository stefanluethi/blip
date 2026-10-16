//! Opening and saving files: with the dialogs of the system natively, and with a file
//! input and a download on the web.

use std::sync::mpsc::{Receiver, Sender, channel};

/// The kinds of files blip opens and saves.
#[derive(Clone, Copy)]
pub enum Kind {
    Recording,
    Layout,
}

impl Kind {
    /// The name of the file type in the dialogs of the system.
    #[cfg(not(target_arch = "wasm32"))]
    fn name(self) -> &'static str {
        match self {
            Kind::Recording => "blip recording",
            Kind::Layout => "blip layout",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Kind::Recording => "blip",
            Kind::Layout => "json",
        }
    }
}

/// A file the user opened.
pub struct File {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The files the user opens, which arrive later on the web.
pub struct Files {
    sender: Sender<Result<File, String>>,
    receiver: Receiver<Result<File, String>>,
}

impl Default for Files {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }
}

impl Files {
    /// A file opened since the last call, or why it could not be read.
    pub fn try_recv(&self) -> Option<Result<File, String>> {
        self.receiver.try_recv().ok()
    }

    /// Lets the user choose a file of `kind` to open, which [`Self::try_recv`] returns.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&self, kind: Kind, _ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(kind.name(), &[kind.extension()])
            .pick_file()
        else {
            return;
        };
        let file = std::fs::read(&path)
            .map(|bytes| File {
                name: path.display().to_string(),
                bytes,
            })
            .map_err(|error| format!("{} cannot be read: {error}", path.display()));
        self.sender.send(file).ok();
    }

    /// Lets the user choose a file of `kind` to open, which [`Self::try_recv`] returns.
    #[cfg(target_arch = "wasm32")]
    pub fn open(&self, kind: Kind, ctx: &egui::Context) {
        if let Err(error) = web::open(kind, self.sender.clone(), ctx.clone()) {
            log::warn!("cannot open a file: {error:?}");
        }
    }

    /// Opens the files dropped on the window.
    pub fn receive_dropped(&self, ctx: &egui::Context) {
        for dropped in ctx.input(|input| input.raw.dropped_files.clone()) {
            let name = dropped.path().display().to_string();
            let read = move |bytes: Result<Vec<u8>, String>| {
                bytes
                    .map(|bytes| File {
                        name: name.clone(),
                        bytes,
                    })
                    .map_err(|error| format!("{name} cannot be read: {error}"))
            };
            #[cfg(not(target_arch = "wasm32"))]
            self.sender.send(read(dropped.bytes())).ok();
            #[cfg(target_arch = "wasm32")]
            {
                let (sender, ctx) = (self.sender.clone(), ctx.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    sender.send(read(dropped.bytes_async().await)).ok();
                    ctx.request_repaint();
                });
            }
        }
    }
}

/// Lets the user save `bytes` as a file of `kind` named `name` by default.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(kind: Kind, name: &str, bytes: &[u8]) -> Result<(), String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(kind.name(), &[kind.extension()])
        .set_file_name(name)
        .save_file()
    else {
        return Ok(());
    };
    std::fs::write(&path, bytes)
        .map_err(|error| format!("{} cannot be written: {error}", path.display()))
}

/// Lets the user save `bytes` as a file of `kind` named `name` by default.
#[cfg(target_arch = "wasm32")]
pub fn save(_kind: Kind, name: &str, bytes: &[u8]) -> Result<(), String> {
    web::download(name, bytes).map_err(|error| format!("the download failed: {error:?}"))
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{File, Kind};
    use std::sync::mpsc::Sender;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    fn document() -> Result<web_sys::Document, JsValue> {
        web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("no document"))
    }

    /// Clicks a file input, sends the chosen file to `sender`.
    pub fn open(
        kind: Kind,
        sender: Sender<Result<File, String>>,
        ctx: egui::Context,
    ) -> Result<(), JsValue> {
        let input: web_sys::HtmlInputElement = document()?.create_element("input")?.dyn_into()?;
        input.set_type("file");
        input.set_accept(&format!(".{}", kind.extension()));
        let chosen = input.clone();
        let on_change = Closure::once_into_js(move || {
            let Some(file) = chosen.files().and_then(|files| files.get(0)) else {
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                let name = file.name();
                let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                let file = read
                    .map(|buffer| File {
                        name: name.clone(),
                        bytes: js_sys::Uint8Array::new(&buffer).to_vec(),
                    })
                    .map_err(|error| format!("{name} cannot be read: {error:?}"));
                sender.send(file).ok();
                ctx.request_repaint();
            });
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Ok(())
    }

    /// Downloads `bytes` as a file named `name`.
    pub fn download(name: &str, bytes: &[u8]) -> Result<(), JsValue> {
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let link: web_sys::HtmlAnchorElement = document()?.create_element("a")?.dyn_into()?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        // the download may still read the blob, so it is released a bit later
        let revoke = Closure::once_into_js(move || {
            web_sys::Url::revoke_object_url(&url).ok();
        });
        web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window"))?
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                revoke.unchecked_ref(),
                10_000,
            )?;
        Ok(())
    }
}
