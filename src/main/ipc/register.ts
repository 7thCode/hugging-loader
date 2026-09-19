import type { BrowserWindow } from 'electron'
import { registerHfHandlers } from './hfHandlers'
import { registerFsHandlers } from './fsHandlers'
import { registerSettingsHandlers } from './settingsHandlers'
import { registerDialogHandlers } from './dialogHandlers'
import { registerGgufHandlers } from './ggufHandlers'

export function registerIpcHandlers(mainWindow: BrowserWindow): void {
  registerHfHandlers()
  registerFsHandlers(mainWindow)
  registerSettingsHandlers()
  registerDialogHandlers(mainWindow)
  registerGgufHandlers()
}
