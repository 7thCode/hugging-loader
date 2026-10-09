import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm as tauriConfirm } from '@tauri-apps/plugin-dialog'
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

// Implements HuggingLoaderApi (installed as `window.api` by main.ts) via Tauri's
// invoke()/listen(). Command names are the snake_case Rust #[tauri::command] fns in
// src-tauri/src/commands/*.rs.
//
// (The earlier Electron build of this app had src/preload/index.ts play the same role via
// contextBridge + ipcRenderer, and main.ts only installed this shim when that hadn't already
// set `window.api` — both gone now that the migration to Tauri is complete.)
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
  },
  confirm: (message: string): Promise<boolean> => tauriConfirm(message)
}

export function installTauriApi(): void {
  window.api = api
}
