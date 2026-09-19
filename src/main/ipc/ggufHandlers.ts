import { ipcMain } from 'electron'
import path from 'path'
import { readGgufHeader } from '../services/ggufParser'
import { loadSettings } from '../services/settingsStore'
import type { GgufHeaderResponse, ReadGgufHeaderRequest } from '../../shared/ipc-types'

// The filename comes from the renderer, so it must never be able to point outside the
// destination folder (e.g. "../../.ssh/id_rsa") or at a non-GGUF file.
export function resolveModelPath(destinationDir: string, filename: string): string {
  const root = path.resolve(destinationDir)
  const resolved = path.resolve(root, filename)
  if (!resolved.startsWith(root + path.sep)) {
    throw new Error('保存先フォルダ外のファイルは読み込めません')
  }
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
