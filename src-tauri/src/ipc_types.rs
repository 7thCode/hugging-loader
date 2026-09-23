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
