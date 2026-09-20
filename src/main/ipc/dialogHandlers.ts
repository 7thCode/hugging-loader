import { ipcMain, dialog } from 'electron'
import type { GetMainWindow } from './register'
import type { ChooseFolderResponse } from '../../shared/ipc-types'

export function registerDialogHandlers(getMainWindow: GetMainWindow): void {
  ipcMain.handle('dialog:chooseFolder', async (): Promise<ChooseFolderResponse> => {
    const options: Electron.OpenDialogOptions = {
      properties: ['openDirectory', 'createDirectory']
    }
    const window = getMainWindow()
    const result = window
      ? await dialog.showOpenDialog(window, options)
      : await dialog.showOpenDialog(options)
    if (result.canceled || result.filePaths.length === 0) {
      return { canceled: true, path: null }
    }
    return { canceled: false, path: result.filePaths[0] }
  })
}
