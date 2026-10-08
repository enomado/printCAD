//! What differs between a desktop and a browser page, kept in one place:
//! starting work beside the window, and reading and writing the user's
//! files.
//!
//! On a desktop each is what the rest of the app did before it went through
//! here: a named thread, the file system. A browser page has one thread and
//! no file system, so work runs where it is asked for and files are the
//! ones the user picked, held in memory, and downloads.

use std::path::{Path, PathBuf};

#[cfg(target_arch = "wasm32")]
pub(crate) mod web;

/// Work started beside the window, which an exit can wait for.
pub(crate) struct Job {
    #[cfg(not(target_arch = "wasm32"))]
    handle: std::thread::JoinHandle<()>,
}

impl Job {
    /// Whether the work is done.
    pub(crate) fn is_finished(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.handle.is_finished();
        #[cfg(target_arch = "wasm32")]
        true
    }

    /// Wait for the work to end.
    pub(crate) fn join(self) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.handle.join();
    }
}

/// Run `work` beside the window: on a thread named `name` on a desktop; on
/// a browser page, which has one thread, now. Either way its answer goes
/// where the caller arranged (a channel read on a later frame), so the
/// caller's side is the same.
/// Whether the app runs on a browser page, which leaves out what needs the
/// machine itself: running other programs, the AI agents, its own folders.
pub(crate) const ON_PAGE: bool = cfg!(target_arch = "wasm32");

/// The application's commands a page cannot carry out, kept out of its
/// menus, palette and keys.
const DESKTOP_ONLY: &[&str] = &["file.send_to_slicer", "app.quit", "app.assistant"];

/// Whether command `id` is one this build carries out.
pub(crate) fn offers(id: &str) -> bool {
    !ON_PAGE || !DESKTOP_ONLY.contains(&id)
}

pub(crate) fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> std::io::Result<Job> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(work)
            .map(|handle| Job { handle })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        work();
        Ok(Job {})
    }
}

/// The bytes of the file at `path`: from the file system on a desktop, from
/// the files the user picked on a browser page.
pub(crate) fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::read(path);
    #[cfg(target_arch = "wasm32")]
    web::read(path)
}

/// Write `bytes` as the file at `path`: to the file system on a desktop; on
/// a browser page, as a download named after the path's file name.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::write(path, bytes);
    #[cfg(target_arch = "wasm32")]
    if web::is_kept(path) {
        web::keep_at(path, bytes);
        Ok(())
    } else {
        web::download(path, bytes)
    }
}

/// Take the file at `path` away; one that is not there is no error.
pub(crate) fn remove(path: &Path) {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::fs::remove_file(path);
    #[cfg(target_arch = "wasm32")]
    web::remove(path);
}

/// The files directly in folder `dir`.
pub(crate) fn list(dir: &Path) -> Vec<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    web::list(dir)
}

/// Whether there is a file at `path`.
pub(crate) fn exists(path: &Path) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return path.is_file();
    #[cfg(target_arch = "wasm32")]
    web::exists(path)
}

/// Where the app keeps its own files when the system names no folder for
/// them: on a page, the files it keeps between visits.
pub(crate) fn kept_dir() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    return None;
    #[cfg(target_arch = "wasm32")]
    Some(PathBuf::from(web::KEPT))
}

/// Say whether any document has edits not saved, which a page asks about
/// before it is closed or left; a desktop asks through its own exit path.
pub(crate) fn set_unsaved(any: bool) {
    #[cfg(target_arch = "wasm32")]
    web::set_unsaved(any);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = any;
}

/// Ask a yes-or-no question; true for yes.
pub(crate) fn confirm(text: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return matches!(
        rfd::MessageDialog::new()
            .set_title("printCAD")
            .set_description(text)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show(),
        rfd::MessageDialogResult::Yes
    );
    #[cfg(target_arch = "wasm32")]
    web_sys::window()
        .and_then(|w| w.confirm_with_message(text).ok())
        .unwrap_or(false)
}

/// Ask for a name on a page, offering `default`; `None` when the user
/// cancels.
#[cfg(target_arch = "wasm32")]
pub(crate) fn ask_name(text: &str, default: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.prompt_with_message_and_default(text, default).ok())
        .flatten()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// What the user chose about unsaved changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unsaved {
    Save,
    Discard,
    Cancel,
}

/// Ask whether to save, discard or keep unsaved changes: the system's
/// dialog on a desktop; the page's own questions in a browser, one at a
/// time, so a cancel never loses work.
pub(crate) fn ask_unsaved(text: &str) -> Unsaved {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
        // GTK and Zenity answer `Custom(label)` for custom buttons, not
        // Yes, No or Cancel.
        const SAVE: &str = "Save";
        const DISCARD: &str = "Discard";
        const CANCEL: &str = "Cancel";
        let answer = MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description(text)
            .set_level(MessageLevel::Warning)
            .set_buttons(MessageButtons::YesNoCancelCustom(
                SAVE.into(),
                DISCARD.into(),
                CANCEL.into(),
            ))
            .show();
        match answer {
            MessageDialogResult::Yes => Unsaved::Save,
            MessageDialogResult::No => Unsaved::Discard,
            MessageDialogResult::Custom(s) if s == SAVE => Unsaved::Save,
            MessageDialogResult::Custom(s) if s == DISCARD => Unsaved::Discard,
            _ => Unsaved::Cancel,
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = text;
        let Some(window) = web_sys::window() else {
            return Unsaved::Cancel;
        };
        let ask = |question: &str| window.confirm_with_message(question).unwrap_or(false);
        if ask("This document has unsaved changes. Download it before going on?") {
            Unsaved::Save
        } else if ask("Discard the unsaved changes?") {
            Unsaved::Discard
        } else {
            Unsaved::Cancel
        }
    }
}

/// Tell the user something they have to acknowledge.
pub(crate) fn warn(title: &str, text: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rfd::{MessageButtons, MessageDialog, MessageLevel};
        let _ = MessageDialog::new()
            .set_title(title)
            .set_description(text)
            .set_level(MessageLevel::Warning)
            .set_buttons(MessageButtons::Ok)
            .show();
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        let _ = window.alert_with_message(&format!("{title}\n\n{text}"));
    }
}

/// The folder for files the app makes for its own short use: the system's
/// temporary folder on a desktop; on a browser page, the page's own files.
pub(crate) fn temp_dir() -> std::path::PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    return std::env::temp_dir();
    #[cfg(target_arch = "wasm32")]
    std::path::PathBuf::from("/browser/tmp")
}

/// Hold `bytes` as a file named `name` that the kernel can read by path:
/// written under the temporary folder on a desktop, kept with the picked
/// files on a browser page.
pub(crate) fn scratch_file(
    folder: &str,
    name: &str,
    bytes: &[u8],
) -> std::io::Result<std::path::PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = temp_dir().join("printcad").join(folder);
        let path = dir.join(name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = folder;
        Ok(web::keep(name, bytes.to_vec()))
    }
}

/// This process's id, which marks the recovery copies it keeps; a browser
/// page has none to ask for, and is one process for the purpose.
pub(crate) fn process_id() -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::process::id();
    // A page has none: one drawn for the visit tells its own autosaved
    // copies from those an earlier visit left.
    #[cfg(target_arch = "wasm32")]
    {
        thread_local! {
            static VISIT: u32 = (js_sys::Math::random() * f64::from(u32::MAX)) as u32;
        }
        VISIT.with(|id| *id)
    }
}
