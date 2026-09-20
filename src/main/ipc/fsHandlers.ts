import { ipcMain } from 'electron'
import * as downloadManager from '../services/downloadManager'
import { checkExists, deleteFile } from '../services/fsUtil'
import { loadSettings } from '../services/settingsStore'
import { removeManifestEntry } from '../services/manifestStore'
import type { GetMainWindow } from './register'
import type {
  CancelDownloadRequest,
  CancelDownloadResponse,
  CheckExistsRequest,
  CheckExistsResponse,
  DeleteFileRequest,
  DeleteFileResponse,
  StartDownloadRequest,
  StartDownloadResponse
} from '../../shared/ipc-types'

export function registerFsHandlers(getMainWindow: GetMainWindow): void {
  ipcMain.handle(
    'fs:checkExists',
    async (_event, req: CheckExistsRequest): Promise<CheckExistsResponse> => {
      const settings = await loadSettings()
      return checkExists(settings.destinationDir, req.filename)
    }
  )

  ipcMain.handle(
    'fs:startDownload',
    async (_event, req: StartDownloadRequest): Promise<StartDownloadResponse> => {
      const settings = await loadSettings()
      const downloadId = await downloadManager.startDownload(
        req.repoId,
        req.filename,
        req.sizeBytes,
        settings.destinationDir,
        { quant: req.quant, paramCount: req.paramCount },
        (event) => {
          // A download keeps running after its window is closed (macOS keeps the app alive);
          // with no window there is simply nobody to tell.
          const window = getMainWindow()
          if (window && !window.isDestroyed()) {
            window.webContents.send('download:progress', event)
          }
        }
      )
      return { downloadId }
    }
  )

  ipcMain.handle(
    'fs:cancelDownload',
    async (_event, req: CancelDownloadRequest): Promise<CancelDownloadResponse> => {
      return { success: downloadManager.cancelDownload(req.downloadId) }
    }
  )

  ipcMain.handle(
    'fs:deleteFile',
    async (_event, req: DeleteFileRequest): Promise<DeleteFileResponse> => {
      const settings = await loadSettings()
      const success = await deleteFile(settings.destinationDir, req.filename)
      if (success) {
        await removeManifestEntry(settings.destinationDir, req.filename)
      }
      return { success }
    }
  )
}
