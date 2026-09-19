import { ipcMain } from 'electron'
import { readGgufHeader } from '../services/ggufParser'
import { resolveInDestination } from '../services/fsUtil'
import { loadSettings } from '../services/settingsStore'
import type { GgufHeaderResponse, ReadGgufHeaderRequest } from '../../shared/ipc-types'

// On top of the destination-folder containment check, only .gguf files may be read.
export function resolveModelPath(destinationDir: string, filename: string): string {
  const resolved = resolveInDestination(destinationDir, filename)
  if (!resolved.toLowerCase().endsWith('.gguf')) {
    throw new Error('.gguf ファイルのみ読み込めます')
  }
  return resolved
}

export function registerGgufHandlers(): void {
  ipcMain.handle(
    'gguf:readHeader',
    async (_event, req: ReadGgufHeaderRequest): Promise<GgufHeaderResponse> => {
      const settings = await loadSettings()
      return readGgufHeader(resolveModelPath(settings.destinationDir, req.filename))
    }
  )
}
