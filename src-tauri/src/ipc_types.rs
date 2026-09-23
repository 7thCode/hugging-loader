//! Mirrors src/shared/ipc-types.ts field-for-field: same JSON shape, `camelCase` on the wire
//! (via `serde(rename_all = "camelCase")`) so the existing TS types on the frontend need no
//! changes. Grows in lockstep with ipc-types.ts as later phases add commands.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub destination_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDestinationDirRequest {
    pub dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChooseFolderResponse {
    pub canceled: bool,
    pub path: Option<String>,
}

/// Mirrors ipc-types.ts's `ParamCountSource` string union — the exact wire values are
/// preserved via `rename_all = "kebab-case"` (GgufMetadata -> "gguf-metadata", etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParamCountSource {
    GgufMetadata,
    RegexEstimate,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub id: String,
    pub author: String,
    pub downloads: i64,
    pub likes: i64,
    pub pipeline_tag: Option<String>,
    pub tags: Vec<String>,
    pub param_count: Option<i64>,
    pub param_count_source: ParamCountSource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub repo_id: String,
    // rfilename: the path within the repo, which may include subfolders — e.g.
    // "llama-2-7b-chat.Q4_K_M.gguf" or "BF16/model-00001-of-00002.gguf". The same relative
    // path is used under the destination folder, so subfolders are preserved on disk.
    pub filename: String,
    pub size_bytes: i64,
    pub quant: Option<String>,
    pub exists_on_disk: bool,
    pub downloaded_at: Option<String>,
    pub param_count: Option<i64>,
    pub param_count_source: ParamCountSource,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchModelsRequest {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub pipeline_tag: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchModelsResponse {
    pub items: Vec<RepoSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesRequest {
    pub repo_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesResponse {
    pub repo_id: String,
    pub files: Vec<FileEntry>,
    pub param_count: Option<i64>,
    pub param_count_source: ParamCountSource,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckExistsRequest {
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckExistsResponse {
    pub exists: bool,
    pub size_bytes: Option<i64>,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFileRequest {
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFileResponse {
    pub success: bool,
}

/// Mirrors ipc-types.ts's `GgufScalar = string | number | boolean`. JS numbers are always
/// float64 regardless of the GGUF source type (uint8 through float64 alike), so `Number(f64)`
/// is the faithful representation here, not a compromise — see services/gguf_parser.rs's
/// `u64_to_scalar`/`i64_to_scalar` for the 2^53 safe-integer string fallback this enables.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum GgufScalar {
    String(String),
    Number(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufArrayInfo {
    pub element_type: String,
    pub length: i64,
    pub preview: Vec<GgufScalar>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufMetadataEntry {
    pub key: String,
    // "type" is a reserved word in Rust; renamed on the wire to match ipc-types.ts exactly.
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: Option<GgufScalar>,
    pub array: Option<GgufArrayInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufTensorInfo {
    pub name: String,
    pub dims: Vec<i64>,
    #[serde(rename = "type")]
    pub type_name: String,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadGgufHeaderRequest {
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufHeaderResponse {
    pub file_size: i64,
    pub version: u32,
    pub tensor_count: i64,
    pub metadata_count: i64,
    pub header_size: i64,
    pub alignment: i64,
    pub data_offset: i64,
    pub file_type_name: Option<String>,
    pub metadata: Vec<GgufMetadataEntry>,
    pub tensors: Vec<GgufTensorInfo>,
}
