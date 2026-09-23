// Ports src/main/services/ggufParser.ts.
//
// Layout and enum values follow gguf-py (llama.cpp/gguf-py/gguf/constants.py), same as the
// original. Runs synchronously on std::fs::File (simpler than an async windowed reader over
// tokio::fs::File) inside `tokio::task::spawn_blocking`, since this is header parsing — bounded
// disk I/O plus CPU work, not something that needs to interleave with other async tasks.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use crate::error::{MapErrString, Result};
use crate::ipc_types::{
    GgufArrayInfo, GgufHeaderResponse, GgufMetadataEntry, GgufScalar, GgufTensorInfo,
};

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const DEFAULT_ALIGNMENT: i64 = 32;
const READ_WINDOW: u64 = 1024 * 1024;
const ARRAY_PREVIEW_LENGTH: u64 = 16;
// Guards against runaway loops on corrupt files; real models are far below these.
const MAX_METADATA_COUNT: u64 = 1_000_000;
const MAX_TENSOR_COUNT: u64 = 1_000_000;
const MAX_TENSOR_DIMS: u32 = 8;

// 64-bit integers beyond 2^53 lose precision as a JS number, so they're kept as decimal strings
// (JS's Number.MAX_SAFE_INTEGER / MIN_SAFE_INTEGER).
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_991;

const VT_UINT8: u32 = 0;
const VT_INT8: u32 = 1;
const VT_UINT16: u32 = 2;
const VT_INT16: u32 = 3;
const VT_UINT32: u32 = 4;
const VT_INT32: u32 = 5;
const VT_FLOAT32: u32 = 6;
const VT_BOOL: u32 = 7;
const VT_STRING: u32 = 8;
const VT_ARRAY: u32 = 9;
const VT_UINT64: u32 = 10;
const VT_INT64: u32 = 11;
const VT_FLOAT64: u32 = 12;

fn value_type_name(value_type: u32) -> String {
    match value_type {
        VT_UINT8 => "uint8",
        VT_INT8 => "int8",
        VT_UINT16 => "uint16",
        VT_INT16 => "int16",
        VT_UINT32 => "uint32",
        VT_INT32 => "int32",
        VT_FLOAT32 => "float32",
        VT_BOOL => "bool",
        VT_STRING => "string",
        VT_ARRAY => "array",
        VT_UINT64 => "uint64",
        VT_INT64 => "int64",
        VT_FLOAT64 => "float64",
        other => return other.to_string(),
    }
    .to_string()
}

fn fixed_value_size(value_type: u32) -> Option<u64> {
    match value_type {
        VT_UINT8 | VT_INT8 | VT_BOOL => Some(1),
        VT_UINT16 | VT_INT16 => Some(2),
        VT_UINT32 | VT_INT32 | VT_FLOAT32 => Some(4),
        VT_UINT64 | VT_INT64 | VT_FLOAT64 => Some(8),
        _ => None,
    }
}

fn ggml_type_name(ggml_type: u32) -> String {
    match ggml_type {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        6 => "Q5_0",
        7 => "Q5_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "IQ2_XXS",
        17 => "IQ2_XS",
        18 => "IQ3_XXS",
        19 => "IQ1_S",
        20 => "IQ4_NL",
        21 => "IQ3_S",
        22 => "IQ2_S",
        23 => "IQ4_XS",
        24 => "I8",
        25 => "I16",
        26 => "I32",
        27 => "I64",
        28 => "F64",
        29 => "IQ1_M",
        30 => "BF16",
        34 => "TQ1_0",
        35 => "TQ2_0",
        39 => "MXFP4",
        40 => "NVFP4",
        41 => "Q1_0",
        42 => "Q2_0",
        other => return format!("type {other}"),
    }
    .to_string()
}

/// `general.file_type` (llama_ftype).
fn file_type_name(file_type: i64) -> Option<&'static str> {
    Some(match file_type {
        0 => "ALL_F32",
        1 => "MOSTLY_F16",
        2 => "MOSTLY_Q4_0",
        3 => "MOSTLY_Q4_1",
        7 => "MOSTLY_Q8_0",
        8 => "MOSTLY_Q5_0",
        9 => "MOSTLY_Q5_1",
        10 => "MOSTLY_Q2_K",
        11 => "MOSTLY_Q3_K_S",
        12 => "MOSTLY_Q3_K_M",
        13 => "MOSTLY_Q3_K_L",
        14 => "MOSTLY_Q4_K_S",
        15 => "MOSTLY_Q4_K_M",
        16 => "MOSTLY_Q5_K_S",
        17 => "MOSTLY_Q5_K_M",
        18 => "MOSTLY_Q6_K",
        19 => "MOSTLY_IQ2_XXS",
        20 => "MOSTLY_IQ2_XS",
        21 => "MOSTLY_Q2_K_S",
        22 => "MOSTLY_IQ3_XS",
        23 => "MOSTLY_IQ3_XXS",
        24 => "MOSTLY_IQ1_S",
        25 => "MOSTLY_IQ4_NL",
        26 => "MOSTLY_IQ3_S",
        27 => "MOSTLY_IQ3_M",
        28 => "MOSTLY_IQ2_S",
        29 => "MOSTLY_IQ2_M",
        30 => "MOSTLY_IQ4_XS",
        31 => "MOSTLY_IQ1_M",
        32 => "MOSTLY_BF16",
        36 => "MOSTLY_TQ1_0",
        37 => "MOSTLY_TQ2_0",
        38 => "MOSTLY_MXFP4_MOE",
        39 => "MOSTLY_NVFP4",
        40 => "MOSTLY_Q1_0",
        41 => "MOSTLY_Q2_0",
        1024 => "GUESSED",
        _ => return None,
    })
}

fn u64_to_scalar(value: u64) -> GgufScalar {
    if value <= MAX_SAFE_INTEGER as u64 {
        GgufScalar::Number(value as f64)
    } else {
        GgufScalar::String(value.to_string())
    }
}

fn i64_to_scalar(value: i64) -> GgufScalar {
    if (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
        GgufScalar::Number(value as f64)
    } else {
        GgufScalar::String(value.to_string())
    }
}

const OOB_ERROR: &str =
    "GGUFヘッダーの途中でファイルが終了しました（破損または途中までのファイルの可能性）";
const READ_ERROR: &str = "GGUFヘッダーの読み込みに失敗しました";

/// Sequential reader over a file that keeps a sliding read window, so a header with a
/// multi-megabyte tokenizer array is parsed without loading the whole file into memory.
struct HeaderReader {
    file: File,
    file_size: u64,
    window: Vec<u8>,
    window_start: u64,
    position: u64,
}

impl HeaderReader {
    fn new(file: File, file_size: u64) -> Self {
        Self {
            file,
            file_size,
            window: Vec::new(),
            window_start: 0,
            position: 0,
        }
    }

    fn read(&mut self, length: u64) -> Result<Vec<u8>> {
        let end = self
            .position
            .checked_add(length)
            .filter(|&e| e <= self.file_size)
            .ok_or(OOB_ERROR)?;
        let in_window = self.position >= self.window_start
            && end <= self.window_start + self.window.len() as u64;
        if !in_window {
            let size = length.max(READ_WINDOW).min(self.file_size - self.position);
            let mut buffer = vec![0u8; size as usize];
            self.file
                .seek(SeekFrom::Start(self.position))
                .map_err_string()?;
            self.file
                .read_exact(&mut buffer)
                .map_err(|_| READ_ERROR.to_string())?;
            self.window = buffer;
            self.window_start = self.position;
        }
        let start = (self.position - self.window_start) as usize;
        let out_end = (end - self.window_start) as usize;
        let out = self.window[start..out_end].to_vec();
        self.position = end;
        Ok(out)
    }

    fn skip(&mut self, length: u64) -> Result<()> {
        let end = self
            .position
            .checked_add(length)
            .filter(|&e| e <= self.file_size)
            .ok_or(OOB_ERROR)?;
        self.position = end;
        Ok(())
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }

    /// The raw 64-bit value, used for byte counts/lengths/dims/offsets — contexts that don't
    /// go into a GgufScalar and so don't need the 2^53 safe-integer string fallback.
    fn u64_raw(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(8)?.try_into().unwrap()))
    }

    /// Strings are never strictly UTF-8-validated (Node's `Buffer.toString('utf-8')` doesn't
    /// throw on invalid sequences either — it substitutes the replacement character).
    fn string(&mut self) -> Result<String> {
        let length = self.u64_raw()?;
        Ok(String::from_utf8_lossy(&self.read(length)?).into_owned())
    }
}

fn read_scalar(reader: &mut HeaderReader, value_type: u32) -> Result<GgufScalar> {
    match value_type {
        VT_UINT8 => Ok(GgufScalar::Number(reader.read(1)?[0] as f64)),
        VT_INT8 => Ok(GgufScalar::Number(reader.read(1)?[0] as i8 as f64)),
        VT_UINT16 => Ok(GgufScalar::Number(
            u16::from_le_bytes(reader.read(2)?.try_into().unwrap()) as f64,
        )),
        VT_INT16 => Ok(GgufScalar::Number(
            i16::from_le_bytes(reader.read(2)?.try_into().unwrap()) as f64,
        )),
        VT_UINT32 => Ok(GgufScalar::Number(reader.u32()? as f64)),
        VT_INT32 => Ok(GgufScalar::Number(
            i32::from_le_bytes(reader.read(4)?.try_into().unwrap()) as f64,
        )),
        VT_FLOAT32 => Ok(GgufScalar::Number(
            f32::from_le_bytes(reader.read(4)?.try_into().unwrap()) as f64,
        )),
        VT_BOOL => Ok(GgufScalar::Bool(reader.read(1)?[0] != 0)),
        VT_STRING => Ok(GgufScalar::String(reader.string()?)),
        VT_UINT64 => Ok(u64_to_scalar(reader.u64_raw()?)),
        VT_INT64 => Ok(i64_to_scalar(i64::from_le_bytes(
            reader.read(8)?.try_into().unwrap(),
        ))),
        VT_FLOAT64 => Ok(GgufScalar::Number(f64::from_le_bytes(
            reader.read(8)?.try_into().unwrap(),
        ))),
        other => Err(format!("未対応のGGUF値タイプです: {other}")),
    }
}

fn read_metadata_entry(reader: &mut HeaderReader) -> Result<GgufMetadataEntry> {
    let key = reader.string()?;
    let value_type = reader.u32()?;

    if value_type != VT_ARRAY {
        let value = read_scalar(reader, value_type)?;
        return Ok(GgufMetadataEntry {
            key,
            type_name: value_type_name(value_type),
            value: Some(value),
            array: None,
        });
    }

    let element_type = reader.u32()?;
    let length = reader.u64_raw()?;
    let element_type_name = value_type_name(element_type);
    let mut preview = Vec::new();

    if let Some(fixed_size) = fixed_value_size(element_type) {
        let preview_count = length.min(ARRAY_PREVIEW_LENGTH);
        for _ in 0..preview_count {
            preview.push(read_scalar(reader, element_type)?);
        }
        reader.skip((length - preview_count) * fixed_size)?;
    } else if element_type == VT_STRING {
        // Strings are length-prefixed, so every element has to be stepped over to reach the
        // next key — unlike fixed-size elements, a jump-skip can't work without reading each
        // element's own length prefix first.
        for i in 0..length {
            let size = reader.u64_raw()?;
            if i < ARRAY_PREVIEW_LENGTH {
                preview.push(GgufScalar::String(
                    String::from_utf8_lossy(&reader.read(size)?).into_owned(),
                ));
            } else {
                reader.skip(size)?;
            }
        }
    } else {
        return Err(format!("未対応の配列要素タイプです: {element_type}"));
    }

    Ok(GgufMetadataEntry {
        key,
        type_name: format!("array<{element_type_name}>"),
        value: None,
        array: Some(GgufArrayInfo {
            element_type: element_type_name,
            length: length as i64,
            preview,
        }),
    })
}

fn read_tensor_info(reader: &mut HeaderReader) -> Result<GgufTensorInfo> {
    let name = reader.string()?;
    let dim_count = reader.u32()?;
    if dim_count > MAX_TENSOR_DIMS {
        return Err(format!("テンソルの次元数が不正です: {dim_count}"));
    }
    let mut dims = Vec::with_capacity(dim_count as usize);
    for _ in 0..dim_count {
        dims.push(reader.u64_raw()? as i64);
    }
    let ggml_type = reader.u32()?;
    let offset = reader.u64_raw()? as i64;
    Ok(GgufTensorInfo {
        name,
        dims,
        type_name: ggml_type_name(ggml_type),
        offset,
    })
}

fn read_gguf_header_sync(file_path: &std::path::Path) -> Result<GgufHeaderResponse> {
    let file = File::open(file_path).map_err_string()?;
    let file_size = file.metadata().map_err_string()?.len();
    let mut reader = HeaderReader::new(file, file_size);

    let magic = reader.read(4)?;
    if magic != GGUF_MAGIC {
        return Err(
            "GGUFファイルではありません（先頭のマジックバイトが \"GGUF\" と一致しません）"
                .to_string(),
        );
    }

    let version = reader.u32()?;
    if version & 0xffff == 0 {
        return Err("ビッグエンディアンのGGUFファイルには対応していません".to_string());
    }
    if version != 2 && version != 3 {
        return Err(format!(
            "未対応のGGUFバージョンです: {version}（対応: 2, 3）"
        ));
    }

    let tensor_count = reader.u64_raw()?;
    let metadata_count = reader.u64_raw()?;
    if tensor_count > MAX_TENSOR_COUNT || metadata_count > MAX_METADATA_COUNT {
        return Err("GGUFヘッダーの件数が不正です（ファイルが破損している可能性）".to_string());
    }

    let mut metadata = Vec::with_capacity(metadata_count as usize);
    for _ in 0..metadata_count {
        metadata.push(read_metadata_entry(&mut reader)?);
    }

    let mut tensors = Vec::with_capacity(tensor_count as usize);
    for _ in 0..tensor_count {
        tensors.push(read_tensor_info(&mut reader)?);
    }

    let header_size = reader.position;

    let alignment = metadata
        .iter()
        .find(|entry| entry.key == "general.alignment")
        .and_then(|entry| match entry.value {
            Some(GgufScalar::Number(n)) if n > 0.0 => Some(n as i64),
            _ => None,
        })
        .unwrap_or(DEFAULT_ALIGNMENT);
    let data_offset = (header_size as f64 / alignment as f64).ceil() as i64 * alignment;

    let file_type_name_str = metadata
        .iter()
        .find(|entry| entry.key == "general.file_type")
        .and_then(|entry| match entry.value {
            Some(GgufScalar::Number(n)) => Some(n as i64),
            _ => None,
        })
        .and_then(file_type_name)
        .map(str::to_string);

    Ok(GgufHeaderResponse {
        file_size: file_size as i64,
        version,
        tensor_count: tensor_count as i64,
        metadata_count: metadata_count as i64,
        header_size: header_size as i64,
        alignment,
        data_offset,
        file_type_name: file_type_name_str,
        metadata,
        tensors,
    })
}

pub async fn read_gguf_header(file_path: PathBuf) -> Result<GgufHeaderResponse> {
    tokio::task::spawn_blocking(move || read_gguf_header_sync(&file_path))
        .await
        .map_err_string()?
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Small helper functions ---

    #[test]
    fn value_type_names_match_gguf_spec() {
        assert_eq!(value_type_name(VT_UINT32), "uint32");
        assert_eq!(value_type_name(VT_STRING), "string");
        assert_eq!(value_type_name(99), "99");
    }

    #[test]
    fn ggml_type_names_match_llama_cpp() {
        assert_eq!(ggml_type_name(12), "Q4_K");
        assert_eq!(ggml_type_name(30), "BF16");
        assert_eq!(ggml_type_name(999), "type 999");
    }

    #[test]
    fn file_type_names_match_llama_ftype() {
        assert_eq!(file_type_name(15), Some("MOSTLY_Q4_K_M"));
        assert_eq!(file_type_name(1024), Some("GUESSED"));
        assert_eq!(file_type_name(9999), None);
    }

    #[test]
    fn u64_scalar_falls_back_to_string_past_safe_integer() {
        assert_eq!(u64_to_scalar(1_000), GgufScalar::Number(1_000.0));
        assert_eq!(
            u64_to_scalar(MAX_SAFE_INTEGER as u64),
            GgufScalar::Number(MAX_SAFE_INTEGER as f64)
        );
        assert_eq!(
            u64_to_scalar(MAX_SAFE_INTEGER as u64 + 1),
            GgufScalar::String((MAX_SAFE_INTEGER as u64 + 1).to_string())
        );
    }

    #[test]
    fn i64_scalar_falls_back_to_string_past_safe_integer_in_both_directions() {
        assert_eq!(i64_to_scalar(-1_000), GgufScalar::Number(-1_000.0));
        assert_eq!(
            i64_to_scalar(MIN_SAFE_INTEGER - 1),
            GgufScalar::String((MIN_SAFE_INTEGER - 1).to_string())
        );
        assert_eq!(
            i64_to_scalar(MAX_SAFE_INTEGER + 1),
            GgufScalar::String((MAX_SAFE_INTEGER + 1).to_string())
        );
    }

    // --- Synthetic-file integration tests ---
    //
    // Hand-assembles minimal-but-real GGUF byte streams and feeds them through
    // read_gguf_header_sync, exercising the full HeaderReader / read_metadata_entry /
    // read_tensor_info pipeline end to end without needing a real model file or network access.

    struct GgufBuilder {
        bytes: Vec<u8>,
    }

    impl GgufBuilder {
        fn new() -> Self {
            let mut b = Self { bytes: Vec::new() };
            b.bytes.extend_from_slice(GGUF_MAGIC);
            b
        }

        fn u32(mut self, v: u32) -> Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn u64(mut self, v: u64) -> Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn i64(mut self, v: i64) -> Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn f32(mut self, v: f32) -> Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn string(self, s: &str) -> Self {
            let bytes = s.as_bytes();
            self.u64(bytes.len() as u64).raw(bytes)
        }

        fn raw(mut self, bytes: &[u8]) -> Self {
            self.bytes.extend_from_slice(bytes);
            self
        }

        /// key + VT_UINT32 type tag + value.
        fn metadata_u32(self, key: &str, value: u32) -> Self {
            self.string(key).u32(VT_UINT32).u32(value)
        }

        fn write_to(self, path: &std::path::Path) {
            std::fs::write(path, self.bytes).unwrap();
        }
    }

    fn temp_gguf_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        (dir, path)
    }

    #[test]
    fn rejects_a_file_with_the_wrong_magic() {
        let (_dir, path) = temp_gguf_path();
        std::fs::write(&path, b"NOPE0000").unwrap();
        let err = read_gguf_header_sync(&path).unwrap_err();
        assert!(err.contains("GGUFファイルではありません"));
    }

    #[test]
    fn rejects_a_big_endian_encoded_version() {
        let (_dir, path) = temp_gguf_path();
        // Big-endian bytes for version=3, read back as LE this becomes 0x03000000 whose low
        // 16 bits are zero — the actual detection mechanism, not just a comment.
        let mut bytes = GGUF_MAGIC.to_vec();
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x03]);
        std::fs::write(&path, bytes).unwrap();
        let err = read_gguf_header_sync(&path).unwrap_err();
        assert!(err.contains("ビッグエンディアン"));
    }

    #[test]
    fn rejects_an_unsupported_version() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new().u32(4).u64(0).u64(0).write_to(&path);
        let err = read_gguf_header_sync(&path).unwrap_err();
        assert!(err.contains("未対応のGGUFバージョンです"));
    }

    #[test]
    fn rejects_a_metadata_count_over_the_guard_cap() {
        let (_dir, path) = temp_gguf_path();
        // The guard fires before any entries are actually read, so no real payload is needed.
        GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(MAX_METADATA_COUNT + 1)
            .write_to(&path);
        let err = read_gguf_header_sync(&path).unwrap_err();
        assert!(err.contains("GGUFヘッダーの件数が不正です"));
    }

    #[test]
    fn parses_a_minimal_valid_header_with_one_metadata_entry_and_one_tensor() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3) // version
            .u64(1) // tensor_count
            .u64(1) // metadata_count
            .metadata_u32("general.quantization_version", 2)
            // tensor: name, dimCount=2, dims=[4,8], ggmlType=12 (Q4_K), offset=0
            .string("token_embd.weight")
            .u32(2)
            .u64(4)
            .u64(8)
            .u32(12)
            .u64(0)
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(result.version, 3);
        assert_eq!(result.tensor_count, 1);
        assert_eq!(result.metadata_count, 1);
        assert_eq!(result.metadata.len(), 1);
        assert_eq!(result.metadata[0].key, "general.quantization_version");
        assert_eq!(result.metadata[0].type_name, "uint32");
        assert_eq!(result.metadata[0].value, Some(GgufScalar::Number(2.0)));
        assert_eq!(result.tensors.len(), 1);
        assert_eq!(result.tensors[0].name, "token_embd.weight");
        assert_eq!(result.tensors[0].dims, vec![4, 8]);
        assert_eq!(result.tensors[0].type_name, "Q4_K");
        assert_eq!(result.tensors[0].offset, 0);
        // Default alignment (32) since no general.alignment entry is present.
        assert_eq!(result.alignment, 32);
        assert_eq!(
            result.data_offset,
            (result.header_size as f64 / 32.0).ceil() as i64 * 32
        );
    }

    #[test]
    fn honors_a_general_alignment_override() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3)
            .u64(0) // no tensors
            .u64(1) // one metadata entry
            .metadata_u32("general.alignment", 64)
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(result.alignment, 64);
        assert_eq!(
            result.data_offset,
            (result.header_size as f64 / 64.0).ceil() as i64 * 64
        );
    }

    #[test]
    fn reads_a_negative_int64_scalar() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(1)
            .string("some.negative_int")
            .u32(VT_INT64)
            .i64(-42)
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(result.metadata[0].value, Some(GgufScalar::Number(-42.0)));
    }

    #[test]
    fn resolves_a_known_file_type_name() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(1)
            .metadata_u32("general.file_type", 15) // MOSTLY_Q4_K_M
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(result.file_type_name, Some("MOSTLY_Q4_K_M".to_string()));
    }

    #[test]
    fn returns_none_file_type_name_for_an_unrecognized_value() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(1)
            .metadata_u32("general.file_type", 9999)
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(result.file_type_name, None);
    }

    #[test]
    fn reads_a_string_scalar_and_a_float32_scalar() {
        let (_dir, path) = temp_gguf_path();
        GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(2)
            .string("general.name")
            .u32(VT_STRING)
            .string("TestModel")
            .string("general.some_float")
            .u32(VT_FLOAT32)
            .f32(1.5)
            .write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        assert_eq!(
            result.metadata[0].value,
            Some(GgufScalar::String("TestModel".to_string()))
        );
        assert_eq!(result.metadata[1].value, Some(GgufScalar::Number(1.5)));
    }

    #[test]
    fn truncates_a_large_fixed_size_array_to_the_preview_length_but_keeps_the_real_count() {
        let (_dir, path) = temp_gguf_path();
        let total: u32 = 20;
        let mut b = GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(1)
            .string("some.array")
            .u32(VT_ARRAY);
        b = b.u32(VT_UINT32).u64(total as u64);
        for i in 0..total {
            b = b.u32(i);
        }
        b.write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        let array = result.metadata[0].array.as_ref().unwrap();
        assert_eq!(array.length, 20);
        assert_eq!(array.preview.len(), 16);
        assert_eq!(array.preview[0], GgufScalar::Number(0.0));
        assert_eq!(array.preview[15], GgufScalar::Number(15.0));
        // Bytes 16..20 must have been correctly skipped, not just truncated mid-stream — the
        // fact parsing completes without an out-of-bounds error confirms the skip byte count
        // ((length - previewCount) * fixedSize) was computed correctly.
    }

    #[test]
    fn reads_a_string_array_preview_and_skips_the_remainder_by_walking_each_length_prefix() {
        let (_dir, path) = temp_gguf_path();
        let mut b = GgufBuilder::new()
            .u32(3)
            .u64(0)
            .u64(1)
            .string("tokenizer.vocab")
            .u32(VT_ARRAY);
        b = b.u32(VT_STRING).u64(18);
        for i in 0..18 {
            b = b.string(&format!("tok{i}"));
        }
        b.write_to(&path);

        let result = read_gguf_header_sync(&path).unwrap();
        let array = result.metadata[0].array.as_ref().unwrap();
        assert_eq!(array.length, 18);
        assert_eq!(array.preview.len(), 16);
        assert_eq!(array.preview[0], GgufScalar::String("tok0".to_string()));
        assert_eq!(array.preview[15], GgufScalar::String("tok15".to_string()));
    }

    #[test]
    fn errors_cleanly_on_a_truncated_file() {
        let (_dir, path) = temp_gguf_path();
        // Claims one metadata entry but the file ends before it's actually written.
        GgufBuilder::new().u32(3).u64(0).u64(1).write_to(&path);
        let err = read_gguf_header_sync(&path).unwrap_err();
        assert!(err.contains("途中でファイルが終了しました"));
    }
}
