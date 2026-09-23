// Mirrors src/main/constants.ts — grows in lockstep with it as later migration phases port
// more of the main-process services (hf_api, chunked_download, ...).

/// Preferred default download destination if it already exists on disk (falls back to the OS
/// Downloads folder otherwise — see services::settings_store::resolve_default_destination).
pub const DEFAULT_DESTINATION_DIR: &str = "/Users/oda/project/llama.cpp/models";
