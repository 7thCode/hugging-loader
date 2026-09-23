import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  CancelDownloadRequest,
  CancelDownloadResponse,
  CheckExistsRequest,
  CheckExistsResponse,
  ChooseFolderResponse,
  DeleteFileRequest,
  DeleteFileResponse,
  DownloadProgressEvent,
  GgufHeaderResponse,
  HuggingLoaderApi,
  ListFilesRequest,
  ListFilesResponse,
  ReadGgufHeaderRequest,
  SearchModelsRequest,
  SearchModelsResponse,
  Settings,
  SetDestinationDirRequest,
  StartDownloadRequest,
  StartDownloadResponse
} from '../../../shared/ipc-types'

// Tauri equivalent of src/preload/index.ts's Electron contextBridge API: same
// HuggingLoaderApi surface, backed by Tauri's invoke()/listen() instead of ipcRenderer.
// Electron's preload script sets `window.api` before any renderer code runs; Tauri has no
// preload step, so main.ts installs this shim itself, only when `window.api` is missing.
//
// Command names mirror the Rust #[tauri::command] fns in src-tauri/src/commands/*.rs
// (snake_case, one per existing IPC channel: hf:searchModels -> hf_search_models, etc).
// Until those commands exist, invoke() rejects with "command <name> not found" — expected
// during the migration, not a bug in this shim.
const api: HuggingLoaderApi = {
  searchModels: (req: SearchModelsRequest): Promise<SearchModelsResponse> =>
    invoke('hf_search_models', { req }),
  listFiles: (req: ListFilesRequest): Promise<ListFilesResponse> =>
    invoke('hf_list_files', { req }),
  checkExists: (req: CheckExistsRequest): Promise<CheckExistsResponse> =>
    invoke('fs_check_exists', { req }),
  startDownload: (req: StartDownloadRequest): Promise<StartDownloadResponse> =>
    invoke('fs_start_download', { req }),
  cancelDownload: (req: CancelDownloadRequest): Promise<CancelDownloadResponse> =>
    invoke('fs_cancel_download', { req }),
  deleteFile: (req: DeleteFileRequest): Promise<DeleteFileResponse> =>
    invoke('fs_delete_file', { req }),
  getSettings: (): Promise<Settings> => invoke('settings_get'),
  setDestinationDir: (req: SetDestinationDirRequest): Promise<Settings> =>
    invoke('settings_set_destination_dir', { req }),
  chooseFolder: (): Promise<ChooseFolderResponse> => invoke('dialog_choose_folder'),
  readGgufHeader: (req: ReadGgufHeaderRequest): Promise<GgufHeaderResponse> =>
    invoke('gguf_read_header', { req }),
  onDownloadProgress: (cb: (event: DownloadProgressEvent) => void): (() => void) => {
    // listen() itself is async (it round-trips to Rust to register), so the unlisten
    // function isn't available synchronously the way ipcRenderer.on/removeListener's was.
    // Buffer that gap: if the caller unsubscribes before the registration resolves, mark it
    // disposed and unlisten as soon as it does instead of leaking the listener.
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<DownloadProgressEvent>('download:progress', (event) => cb(event.payload)).then(
      (fn) => {
        if (disposed) fn()
        else unlisten = fn
      }
    )
    return () => {
      disposed = true
      unlisten?.()
    }
  }
}

export function installTauriApi(): void {
  window.api = api
}
