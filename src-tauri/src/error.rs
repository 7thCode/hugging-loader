//! `#[tauri::command]` functions return `Result<T, String>` (see src-tauri/src/commands/*.rs):
//! the `Err` string becomes the rejection message `invoke()` throws on the frontend, mirroring
//! how the Electron main process used to throw plain `Error(message)` from its IPC handlers
//! (src/main/ipc/*.ts) and let `ipcRenderer.invoke`'s rejection carry that message verbatim.
//!
//! `MapErrString` lets service code use `?` on `std::io::Error`, `serde_json::Error`, etc. and
//! land on that same `String` error type without writing `.map_err(|e| e.to_string())` at every
//! call site.

pub type Result<T> = std::result::Result<T, String>;

pub trait MapErrString<T> {
    fn map_err_string(self) -> Result<T>;
}

impl<T, E: std::fmt::Display> MapErrString<T> for std::result::Result<T, E> {
    fn map_err_string(self) -> Result<T> {
        self.map_err(|e| e.to_string())
    }
}
