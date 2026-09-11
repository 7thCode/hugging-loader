import { contextBridge, ipcRenderer } from 'electron'
import { electronAPI } from '@electron-toolkit/preload'
import type {
  CancelDownloadRequest,
  CancelDownloadResponse,
  CheckExistsRequest,
  CheckExistsResponse,
  ChooseFolderResponse,
  DeleteFileRequest,
  DeleteFileResponse,
  DownloadProgressEvent,
  HuggingLoaderApi,
  ListFilesRequest,
  ListFilesResponse,
  SearchModelsRequest,
  SearchModelsResponse,
  Settings,
  SetDestinationDirRequest,
  StartDownloadRequest,
  StartDownloadResponse
} from '../shared/ipc-types'

const api: HuggingLoaderApi = {
  searchModels: (req: SearchModelsRequest): Promise<SearchModelsResponse> =>
    ipcRenderer.invoke('hf:searchModels', req),
  listFiles: (req: ListFilesRequest): Promise<ListFilesResponse> =>
    ipcRenderer.invoke('hf:listFiles', req),
  checkExists: (req: CheckExistsRequest): Promise<CheckExistsResponse> =>
    ipcRenderer.invoke('fs:checkExists', req),
  startDownload: (req: StartDownloadRequest): Promise<StartDownloadResponse> =>
    ipcRenderer.invoke('fs:startDownload', req),
  cancelDownload: (req: CancelDownloadRequest): Promise<CancelDownloadResponse> =>
    ipcRenderer.invoke('fs:cancelDownload', req),
  deleteFile: (req: DeleteFileRequest): Promise<DeleteFileResponse> =>
    ipcRenderer.invoke('fs:deleteFile', req),
  getSettings: (): Promise<Settings> => ipcRenderer.invoke('settings:get'),
  setDestinationDir: (req: SetDestinationDirRequest): Promise<Settings> =>
    ipcRenderer.invoke('settings:setDestinationDir', req),
  chooseFolder: (): Promise<ChooseFolderResponse> => ipcRenderer.invoke('dialog:chooseFolder'),
  onDownloadProgress: (cb: (event: DownloadProgressEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, payload: DownloadProgressEvent): void =>
      cb(payload)
    ipcRenderer.on('download:progress', listener)
    return () => ipcRenderer.removeListener('download:progress', listener)
  }
}

// Use `contextBridge` APIs to expose Electron APIs to
// renderer only if context isolation is enabled, otherwise
// just add to the DOM global.
if (process.contextIsolated) {
  try {
    contextBridge.exposeInMainWorld('electron', electronAPI)
    contextBridge.exposeInMainWorld('api', api)
  } catch (error) {
    console.error(error)
  }
} else {
  // @ts-ignore (define in dts)
  window.electron = electronAPI
  // @ts-ignore (define in dts)
  window.api = api
}
