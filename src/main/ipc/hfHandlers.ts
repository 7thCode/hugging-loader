import { ipcMain } from 'electron'
import { searchRepos, fetchRepoFiles } from '../services/hfApiClient'
import { findExistingFilenames } from '../services/fsUtil'
import { loadSettings } from '../services/settingsStore'
import { loadManifest } from '../services/manifestStore'
import type {
  ListFilesRequest,
  ListFilesResponse,
  SearchModelsRequest,
  SearchModelsResponse
} from '../../shared/ipc-types'

export function registerHfHandlers(): void {
  ipcMain.handle(
    'hf:searchModels',
    async (_event, req: SearchModelsRequest): Promise<SearchModelsResponse> => {
      return searchRepos(req)
    }
  )

  ipcMain.handle(
    'hf:listFiles',
    async (_event, req: ListFilesRequest): Promise<ListFilesResponse> => {
      const result = await fetchRepoFiles(req.repoId)
      const settings = await loadSettings()
      const [existing, manifest] = await Promise.all([
        findExistingFilenames(
          settings.destinationDir,
          result.files.map((f) => f.filename)
        ),
        loadManifest(settings.destinationDir)
      ])
      return {
        repoId: result.repoId,
        paramCount: result.paramCount,
        paramCountSource: result.paramCountSource,
        files: result.files.map((f) => {
          const onDisk = existing.has(f.filename)
          const manifestEntry = manifest[f.filename]
          // If the manifest attributes this filename to a different repo, don't claim it
          // for this repo — two repos can legitimately publish an identically-named file.
          const existsOnDisk = onDisk && (!manifestEntry || manifestEntry.repoId === f.repoId)
          return {
            ...f,
            existsOnDisk,
            downloadedAt: existsOnDisk ? (manifestEntry?.downloadedAt ?? null) : null
          }
        })
      }
    }
  )
}
