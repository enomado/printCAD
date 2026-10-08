use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct NativeFile {
    path: PathBuf,
}

impl From<PathBuf> for NativeFile {
    fn from(path: PathBuf) -> Self {
        Self { path }
    }
}

impl egui::DroppedFile for NativeFile {
    fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.path).map_err(|err| err.to_string())
    }

    // A browser hands over no paths, so nothing makes one of these there;
    // the trait asks for the asynchronous read on that target.
    #[cfg(target_arch = "wasm32")]
    fn bytes_async(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + '_>> {
        Box::pin(async { Err("a browser hands over no file paths".to_owned()) })
    }
}
