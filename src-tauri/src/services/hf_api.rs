// Ports src/main/services/hfApiClient.ts.

use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::constants::{HF_API_BASE, SEARCH_PAGE_LIMIT};
use crate::error::{MapErrString, Result};
use crate::ipc_types::{
    FileEntry, ListFilesResponse, ParamCountSource, RepoSummary, SearchModelsRequest,
    SearchModelsResponse,
};
use crate::services::{param_count, quant_parser};

#[derive(Debug, Deserialize)]
struct HfSearchItem {
    id: String,
    author: Option<String>,
    downloads: Option<i64>,
    likes: Option<i64>,
    pipeline_tag: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct HfGgufInfo {
    total: Option<i64>,
}

// fetchRepoFiles() in hfApiClient.ts declares author/downloads/likes/id/pipeline_tag on this
// shape too (mirroring the search endpoint's item shape and its own return type, which
// includes `pipelineTag`) but ListFilesResponse — the actual IPC contract, and the only part
// of fetchRepoFiles()'s result hfHandlers.ts's `hf:listFiles` handler reads — has no
// pipelineTag field, so it never ends up used. Trimmed here to just tags/siblings/gguf, the
// fields that do feed ListFilesResponse; unread fields would otherwise trip Rust's dead_code
// lint.
#[derive(Debug, Deserialize)]
struct HfModelInfo {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    gguf: Option<HfGgufInfo>,
}

fn parse_next_cursor(link_header: Option<&str>) -> Option<String> {
    static LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<([^>]+)>;\s*rel="next""#).expect("link regex is well-formed")
    });
    let header = link_header?;
    let url_str = LINK_RE.captures(header)?.get(1)?.as_str();
    let url = url::Url::parse(url_str).ok()?;
    url.query_pairs()
        .find(|(k, _)| k == "cursor")
        .map(|(_, v)| v.into_owned())
}

pub async fn search_repos(
    client: &reqwest::Client,
    req: &SearchModelsRequest,
) -> Result<SearchModelsResponse> {
    let mut params: Vec<(&str, String)> = Vec::new();
    if let Some(search) = req.search.as_deref().filter(|s| !s.is_empty()) {
        params.push(("search", search.to_string()));
    }
    params.push(("filter", "gguf".to_string()));
    if let Some(tag) = req.pipeline_tag.as_deref().filter(|s| !s.is_empty()) {
        params.push(("pipeline_tag", tag.to_string()));
    }
    params.push(("sort", "downloads".to_string()));
    params.push(("direction", "-1".to_string()));
    params.push(("limit", req.limit.unwrap_or(SEARCH_PAGE_LIMIT).to_string()));
    if let Some(cursor) = req.cursor.as_deref().filter(|s| !s.is_empty()) {
        params.push(("cursor", cursor.to_string()));
    }

    let res = client
        .get(HF_API_BASE)
        .query(&params)
        .send()
        .await
        .map_err_string()?;
    if !res.status().is_success() {
        return Err(format!(
            "Hugging Face search failed: {} {}",
            res.status().as_u16(),
            res.status().canonical_reason().unwrap_or("")
        ));
    }
    let link_header = res
        .headers()
        .get(reqwest::header::LINK)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let items: Vec<HfSearchItem> = res.json().await.map_err_string()?;
    let next_cursor = parse_next_cursor(link_header.as_deref());

    let repos = items
        .into_iter()
        .map(|item| {
            let author = item
                .author
                .unwrap_or_else(|| item.id.split('/').next().unwrap_or("").to_string());
            RepoSummary {
                id: item.id,
                author,
                downloads: item.downloads.unwrap_or(0),
                likes: item.likes.unwrap_or(0),
                pipeline_tag: item.pipeline_tag,
                tags: item.tags,
                param_count: None,
                param_count_source: ParamCountSource::Unknown,
            }
        })
        .collect();

    Ok(SearchModelsResponse {
        items: repos,
        next_cursor,
    })
}

/// Files are returned un-annotated (`existsOnDisk: false`, `downloadedAt: None`) — mirroring
/// fetchRepoFiles.ts's own comment ("annotated by the fs layer in the IPC handler"). The
/// `hf_list_files` command (commands/hf.rs) fills those in against the destination folder and
/// manifest, same as hfHandlers.ts's `hf:listFiles` handler does around fetchRepoFiles().
pub async fn fetch_repo_files(
    client: &reqwest::Client,
    repo_id: &str,
) -> Result<ListFilesResponse> {
    let url = format!("{HF_API_BASE}/{repo_id}?blobs=true");
    let res = client.get(&url).send().await.map_err_string()?;
    if !res.status().is_success() {
        return Err(format!(
            "Hugging Face model info failed for {repo_id}: {} {}",
            res.status().as_u16(),
            res.status().canonical_reason().unwrap_or("")
        ));
    }
    let info: HfModelInfo = res.json().await.map_err_string()?;

    let (param_count, param_count_source) =
        param_count::resolve_param_count(repo_id, &info.tags, info.gguf.and_then(|g| g.total));

    let files = info
        .siblings
        .into_iter()
        .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
        .map(|s| FileEntry {
            repo_id: repo_id.to_string(),
            quant: quant_parser::parse_quant(&s.rfilename),
            filename: s.rfilename,
            size_bytes: s.size.unwrap_or(0),
            exists_on_disk: false,
            downloaded_at: None,
            param_count,
            param_count_source,
        })
        .collect();

    Ok(ListFilesResponse {
        repo_id: repo_id.to_string(),
        files,
        param_count,
        param_count_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cursor_from_a_next_link_header() {
        let header = r#"<https://huggingface.co/api/models?cursor=abc123&limit=20>; rel="next""#;
        assert_eq!(parse_next_cursor(Some(header)), Some("abc123".to_string()));
    }

    #[test]
    fn ignores_a_rel_prev_link() {
        let header = r#"<https://huggingface.co/api/models?cursor=abc123>; rel="prev""#;
        assert_eq!(parse_next_cursor(Some(header)), None);
    }

    #[test]
    fn returns_none_when_header_is_absent() {
        assert_eq!(parse_next_cursor(None), None);
    }
}
