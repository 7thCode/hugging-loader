import type { BrowserWindow } from 'electron'
import { registerHfHandlers } from './hfHandlers'
import { registerFsHandlers } from './fsHandlers'
import { registerSettingsHandlers } from './settingsHandlers'
import { registerDialogHandlers } from './dialogHandlers'
import { registerGgufHandlers } from './ggufHandlers'

// Must be called exactly once per process: ipcMain.handle throws if a channel is registered
// twice. On macOS the app outlives its window (closing the window doesn't quit), and a new
// window is created on `activate`, so handlers can't be tied to any one window. They receive a
// getter for whichever window is current instead.
export type GetMainWindow = () => BrowserWindow | null

export function registerIpcHandlers(getMainWindow: GetMainWindow): void {
  registerHfHandlers()
  registerFsHandlers(getMainWindow)
  registerSettingsHandlers()
  registerDialogHandlers(getMainWindow)
  registerGgufHandlers()
}
