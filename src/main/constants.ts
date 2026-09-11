export const DEFAULT_DESTINATION_DIR = '/Users/oda/project/llama.cpp/models'

export const HF_API_BASE = 'https://huggingface.co/api/models'
export const HF_RESOLVE_BASE = 'https://huggingface.co'

// Bound the number of repos fetched per search page, and the number of
// concurrent per-repo file-listing requests, to stay polite to the HF API.
export const SEARCH_PAGE_LIMIT = 20
export const FILE_LISTING_CONCURRENCY = 4

// Parallel segmented downloading: split a file into this many byte-range chunks
// and fetch them concurrently. Files smaller than MIN_CHUNK_SIZE_BYTES are
// downloaded as a single chunk (splitting overhead isn't worth it).
export const DOWNLOAD_CHUNK_COUNT = 6
export const MIN_CHUNK_SIZE_BYTES = 8 * 1024 * 1024
