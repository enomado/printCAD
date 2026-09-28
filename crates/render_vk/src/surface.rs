use ash::{Entry, Instance, ext, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::RenderError;

pub fn create_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, RenderError> {
    let display = window
        .display_handle()
        .map_err(|e| RenderError::Initialization(format!("display handle error: {e}")))?;
    let handle = window
        .window_handle()
        .map_err(|e| RenderError::Initialization(format!("window handle error: {e}")))?;
    // The handles are the live window's, which outlives the surface: the
    // renderer is dropped before the window.
    unsafe { ash_window::create_surface(entry, instance, display.as_raw(), handle.as_raw(), None) }
        .map_err(RenderError::from)
}

/// The instance extensions a surface on this window needs, and the debug
/// messenger's when validation is on.
pub fn required_extensions(
    window: &Window,
    enable_validation: bool,
) -> Result<Vec<*const i8>, RenderError> {
    let display = window
        .display_handle()
        .map_err(|e| RenderError::Initialization(format!("display handle error: {e}")))?;
    let mut extensions = ash_window::enumerate_required_extensions(display.as_raw())
        .map_err(|e| {
            RenderError::UnsupportedPlatform(format!("windowing platform not supported: {e}"))
        })?
        .to_vec();
    if enable_validation {
        extensions.push(ext::debug_utils::NAME.as_ptr());
    }
    Ok(extensions)
}
