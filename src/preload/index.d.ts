import { ElectronAPI } from '@electron-toolkit/preload'
import type { HuggingLoaderApi } from '../shared/ipc-types'

declare global {
  interface Window {
    electron: ElectronAPI
    api: HuggingLoaderApi
  }
}
