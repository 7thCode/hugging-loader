// Mirrors src/main/constants.ts — grows in lockstep with it as later migration phases port
// more of the main-process services (hf_api, chunked_download, ...).

/// Preferred default download destination if it already exists on disk (falls back to the OS
/// Downloads folder otherwise — see services::settings_store::resolve_default_destination).
pub const DEFAULT_DESTINATION_DIR: &str = "/Users/oda/project/llama.cpp/models";

pub const HF_API_BASE: &str = "https://huggingface.co/api/models";
pub const HF_RESOLVE_BASE: &str = "https://huggingface.co";

/// Bound the number of repos fetched per search page.
pub const SEARCH_PAGE_LIMIT: u32 = 20;

/// Parallel segmented downloading: split a file into this many byte-range chunks and fetch
/// them concurrently. Files smaller than MIN_CHUNK_SIZE_BYTES are downloaded as a single
/// chunk (splitting overhead isn't worth it).
pub const DOWNLOAD_CHUNK_COUNT: usize = 6;
pub const MIN_CHUNK_SIZE_BYTES: u64 = 8 * 1024 * 1024;
