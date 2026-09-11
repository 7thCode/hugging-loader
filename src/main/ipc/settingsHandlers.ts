import { ipcMain } from 'electron'
import { loadSettings, setDestinationDir } from '../services/settingsStore'
import type { Settings, SetDestinationDirRequest } from '../../shared/ipc-types'

export function registerSettingsHandlers(): void {
  ipcMain.handle('settings:get', async (): Promise<Settings> => {
    return loadSettings()
  })

  ipcMain.handle(
    'settings:setDestinationDir',
    async (_event, req: SetDestinationDirRequest): Promise<Settings> => {
      return setDestinationDir(req.dir)
    }
  )
}
