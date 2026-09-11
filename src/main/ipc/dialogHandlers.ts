import { ipcMain, dialog, type BrowserWindow } from 'electron'
import type { ChooseFolderResponse } from '../../shared/ipc-types'

export function registerDialogHandlers(mainWindow: BrowserWindow): void {
  ipcMain.handle('dialog:chooseFolder', async (): Promise<ChooseFolderResponse> => {
    const result = await dialog.showOpenDialog(mainWindow, {
      properties: ['openDirectory', 'createDirectory']
    })
    if (result.canceled || result.filePaths.length === 0) {
      return { canceled: true, path: null }
    }
    return { canceled: false, path: result.filePaths[0] }
  })
}
