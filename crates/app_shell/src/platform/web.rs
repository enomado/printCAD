//! A browser page's files: the ones its user picked, held in memory under
//! a path of their own, and downloads for what the app writes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

/// Where picked files appear to live: a folder no desktop path starts with.
const ROOT: &str = "/browser";

thread_local! {
    /// Every file picked this session, by the path it was given.
    static FILES: RefCell<HashMap<PathBuf, Vec<u8>>> = RefCell::new(HashMap::new());
}

/// Hold `bytes` as the file `name`; the path it is read back by.
pub(crate) fn keep(name: &str, bytes: Vec<u8>) -> PathBuf {
    let path = Path::new(ROOT).join(name);
    FILES.with(|files| files.borrow_mut().insert(path.clone(), bytes));
    path
}

/// The bytes of a picked file.
pub(crate) fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    FILES
        .with(|files| files.borrow().get(path).cloned())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} was not picked in this page", path.display()),
            )
        })
}

fn js_error(what: &str, error: wasm_bindgen::JsValue) -> std::io::Error {
    std::io::Error::other(format!("{what}: {error:?}"))
}

/// Hand `bytes` to the browser as a download named after `path`'s file
/// name. The file a later read asks for by the same path is these bytes.
pub(crate) fn download(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "printcad-file".to_string());
    FILES.with(|files| {
        files
            .borrow_mut()
            .insert(path.to_path_buf(), bytes.to_vec())
    });
    let window = web_sys::window().ok_or_else(|| std::io::Error::other("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| std::io::Error::other("no document"))?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|e| js_error("could not make the file", e))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| js_error("could not offer the file", e))?;
    let link: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| js_error("could not offer the file", e))?
        .unchecked_into();
    link.set_href(&url);
    link.set_download(&name);
    link.click();
    // The browser has started the download; the address may go.
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Ask the user for files through the page's own picker: `accept` lists
/// the extensions offered (`.step,.stp`), `many` allows several. `done`
/// gets the paths they are held under once read, or nothing when the
/// picker was closed.
pub(crate) fn pick(accept: &str, many: bool, done: impl FnOnce(Vec<PathBuf>) + 'static) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        done(Vec::new());
        return;
    };
    let Ok(input) = document.create_element("input") else {
        done(Vec::new());
        return;
    };
    let input: web_sys::HtmlInputElement = input.unchecked_into();
    input.set_type("file");
    input.set_accept(accept);
    input.set_multiple(many);

    // The picker answers once: with files (`change`) or without (`cancel`).
    let done = std::rc::Rc::new(RefCell::new(Some(done)));
    let finish = {
        let done = done.clone();
        move |paths: Vec<PathBuf>| {
            if let Some(done) = done.borrow_mut().take() {
                done(paths);
            }
        }
    };
    let on_change = {
        let input = input.clone();
        let finish = finish.clone();
        Closure::<dyn FnMut()>::new(move || {
            let files = input.files();
            let finish = finish.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut paths = Vec::new();
                if let Some(files) = files {
                    for i in 0..files.length() {
                        let Some(file) = files.get(i) else { continue };
                        let Ok(buffer) =
                            wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                        else {
                            tracing::warn!("could not read {}", file.name());
                            continue;
                        };
                        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                        paths.push(keep(&file.name(), bytes));
                    }
                }
                finish(paths);
            });
        })
    };
    let on_cancel = Closure::<dyn FnMut()>::new(move || finish(Vec::new()));
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    let _ = input.add_event_listener_with_callback("cancel", on_cancel.as_ref().unchecked_ref());
    // The page keeps the handlers for as long as the input can call them.
    on_change.forget();
    on_cancel.forget();
    input.click();
}

/// Hold `bytes` as the file at `path`: a picked file handed to a kernel
/// worker, which has a store of its own.
pub(crate) fn put(path: &Path, bytes: Vec<u8>) {
    FILES.with(|files| files.borrow_mut().insert(path.to_path_buf(), bytes));
}
