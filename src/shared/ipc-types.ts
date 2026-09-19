// Shared IPC contract types, imported by main, preload, and renderer.

export type ParamCountSource = 'gguf-metadata' | 'regex-estimate' | 'unknown'

export interface RepoSummary {
  id: string // e.g. "bartowski/Meta-Llama-3.1-8B-Instruct-GGUF"
  author: string
  downloads: number
  likes: number
  pipelineTag: string | null
  tags: string[]
  paramCount: number | null // raw parameter count (not bucketed)
  paramCountSource: ParamCountSource
}

export interface FileEntry {
  repoId: string
  // rfilename: the path within the repo, which may include subfolders — e.g.
  // "llama-2-7b-chat.Q4_K_M.gguf" or "BF16/model-00001-of-00002.gguf". The same relative
  // path is used under the destination folder, so subfolders are preserved on disk.
  filename: string
  sizeBytes: number
  quant: string | null // e.g. "Q4_K_M", or null if unrecognized
  existsOnDisk: boolean
  // Set when a manifest record exists for this exact (repoId, filename) pair — i.e. this
  // app downloaded it. null when the file isn't downloaded, or exists on disk without a
  // manifest record (e.g. placed there manually, or downloaded before this feature existed).
  downloadedAt: string | null
  // Denormalized from the repo (all files in a repo share the same values);
  // only resolved once hf:listFiles has fetched the repo's ?blobs=true metadata.
  paramCount: number | null
  paramCountSource: ParamCountSource
}

export interface SearchModelsRequest {
  search?: string
  pipelineTag?: string | null
  cursor?: string | null
  limit?: number
}

export interface SearchModelsResponse {
  items: RepoSummary[]
  nextCursor: string | null
}

export interface ListFilesRequest {
  repoId: string
}

export interface ListFilesResponse {
  repoId: string
  files: FileEntry[]
  // Repo-level metadata resolved together with the file listing (single ?blobs=true call);
  // returned here too so the renderer can update the repo's RepoSummary display.
  paramCount: number | null
  paramCountSource: ParamCountSource
}

export interface CheckExistsRequest {
  filename: string
}

export interface CheckExistsResponse {
  exists: boolean
  sizeBytes: number | null
  path: string
}

export interface StartDownloadRequest {
  repoId: string
  filename: string
  sizeBytes: number
  quant: string | null
  paramCount: number | null
}

export interface StartDownloadResponse {
  downloadId: string
}

export interface CancelDownloadRequest {
  downloadId: string
}

export interface CancelDownloadResponse {
  success: boolean
}

export interface DeleteFileRequest {
  filename: string
}

export interface DeleteFileResponse {
  success: boolean
}

export type DownloadState = 'downloading' | 'completed' | 'error' | 'canceled'

export interface DownloadProgressEvent {
  downloadId: string
  repoId: string
  filename: string
  receivedBytes: number
  totalBytes: number
  percent: number // 0-100
  state: DownloadState
  errorMessage?: string
}

export type GgufScalar = string | number | boolean

export interface GgufMetadataEntry {
  key: string
  type: string // e.g. "uint32", "string", "array<string>"
  value: GgufScalar | null // null for arrays
  // Arrays (e.g. the tokenizer vocabulary) can hold hundreds of thousands of elements,
  // so only their length and the first few elements are sent to the renderer.
  array: { elementType: string; length: number; preview: GgufScalar[] } | null
}

export interface GgufTensorInfo {
  name: string
  dims: number[]
  type: string // ggml tensor type name, e.g. "Q4_K"
  offset: number // byte offset within the tensor data section
}

export interface ReadGgufHeaderRequest {
  filename: string
}

export interface GgufHeaderResponse {
  fileSize: number
  version: number
  tensorCount: number
  metadataCount: number
  headerSize: number // bytes from the start of the file to the end of the tensor info table
  alignment: number
  dataOffset: number // where the tensor data section starts (headerSize rounded up to alignment)
  fileTypeName: string | null // llama_ftype name for `general.file_type`, e.g. "MOSTLY_Q4_K_M"
  metadata: GgufMetadataEntry[]
  tensors: GgufTensorInfo[]
}

export interface Settings {
  destinationDir: string
}

export interface SetDestinationDirRequest {
  dir: string
}

export interface ChooseFolderResponse {
  canceled: boolean
  path: string | null
}

// Buckets applied to a resolved parameter count (raw units, not billions).
export const PARAM_COUNT_BUCKETS = [
  { id: 'lt-1b', label: '< 1B', min: 0, max: 1_000_000_000 },
  { id: '1-4b', label: '1B - 4B', min: 1_000_000_000, max: 4_000_000_000 },
  { id: '4-9b', label: '4B - 9B', min: 4_000_000_000, max: 9_000_000_000 },
  { id: '9-20b', label: '9B - 20B', min: 9_000_000_000, max: 20_000_000_000 },
  { id: '20-45b', label: '20B - 45B', min: 20_000_000_000, max: 45_000_000_000 },
  { id: '45-90b', label: '45B - 90B', min: 45_000_000_000, max: 90_000_000_000 },
  { id: 'gt-90b', label: '90B+', min: 90_000_000_000, max: Infinity },
  { id: 'unknown', label: '不明', min: null, max: null }
] as const

export type ParamCountBucketId = (typeof PARAM_COUNT_BUCKETS)[number]['id']

export function bucketForParamCount(paramCount: number | null): ParamCountBucketId {
  if (paramCount === null) return 'unknown'
  const bucket = PARAM_COUNT_BUCKETS.find(
    (b) => b.min !== null && b.max !== null && paramCount >= b.min && paramCount < b.max
  )
  return bucket ? bucket.id : 'unknown'
}

// K-quants are named `Q<level>_K` or `Q<level>_K_<SUFFIX>` (S/M/L, plus Unsloth's extended
// XL/XXL "dynamic" quants, e.g. "Q8_K_XL"). Generated rather than enumerated by hand so new
// suffix combinations from quantizers (Unsloth, bartowski, ...) are recognized automatically.
const K_QUANT_LEVELS = [2, 3, 4, 5, 6, 8] as const
const K_QUANT_SUFFIXES = ['', '_S', '_M', '_L', '_XL', '_XXL'] as const
const K_QUANT_TYPES = K_QUANT_LEVELS.flatMap((level) =>
  K_QUANT_SUFFIXES.map((suffix) => `Q${level}_K${suffix}`)
)

// Common llama.cpp GGUF quantization types, for the filter checkbox list and (in
// quantParser.ts) as the vocabulary a filename is matched against.
export const QUANT_TYPES: string[] = [
  'F32',
  'F16',
  'BF16',
  'Q8_0',
  'Q5_1',
  'Q5_0',
  'Q4_1',
  'Q4_0',
  ...K_QUANT_TYPES,
  'IQ4_XS',
  'IQ4_NL',
  'IQ3_XXS',
  'IQ3_XS',
  'IQ3_S',
  'IQ3_M',
  'IQ2_XXS',
  'IQ2_XS',
  'IQ2_S',
  'IQ2_M',
  'IQ1_S',
  'IQ1_M',
  'TQ1_0',
  'TQ2_0'
]

// Common pipeline tags seen on GGUF repos, for the task filter dropdown.
export const COMMON_PIPELINE_TAGS = [
  'text-generation',
  'text2text-generation',
  'image-text-to-image',
  'image-text-to-text',
  'feature-extraction',
  'text-to-speech',
  'automatic-speech-recognition',
  'audio-text-to-text',
  'any-to-any'
] as const

export interface HuggingLoaderApi {
  searchModels: (req: SearchModelsRequest) => Promise<SearchModelsResponse>
  listFiles: (req: ListFilesRequest) => Promise<ListFilesResponse>
  checkExists: (req: CheckExistsRequest) => Promise<CheckExistsResponse>
  startDownload: (req: StartDownloadRequest) => Promise<StartDownloadResponse>
  cancelDownload: (req: CancelDownloadRequest) => Promise<CancelDownloadResponse>
  deleteFile: (req: DeleteFileRequest) => Promise<DeleteFileResponse>
  getSettings: () => Promise<Settings>
  setDestinationDir: (req: SetDestinationDirRequest) => Promise<Settings>
  chooseFolder: () => Promise<ChooseFolderResponse>
  readGgufHeader: (req: ReadGgufHeaderRequest) => Promise<GgufHeaderResponse>
  onDownloadProgress: (cb: (event: DownloadProgressEvent) => void) => () => void
}
