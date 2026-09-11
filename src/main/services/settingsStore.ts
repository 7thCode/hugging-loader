import { app } from 'electron'
import { promises as fs } from 'fs'
import path from 'path'
import { DEFAULT_DESTINATION_DIR } from '../constants'
import type { Settings } from '../../shared/ipc-types'

function configPath(): string {
  return path.join(app.getPath('userData'), 'config.json')
}

async function directoryExists(dir: string): Promise<boolean> {
  try {
    const stat = await fs.stat(dir)
    return stat.isDirectory()
  } catch {
    return false
  }
}

async function resolveDefaultDestination(): Promise<string> {
  if (await directoryExists(DEFAULT_DESTINATION_DIR)) {
    return DEFAULT_DESTINATION_DIR
  }
  return app.getPath('downloads')
}

let cached: Settings | null = null

export async function loadSettings(): Promise<Settings> {
  if (cached) return cached

  try {
    const raw = await fs.readFile(configPath(), 'utf-8')
    const parsed = JSON.parse(raw) as Partial<Settings>
    if (parsed && typeof parsed.destinationDir === 'string' && parsed.destinationDir.length > 0) {
      cached = { destinationDir: parsed.destinationDir }
      return cached
    }
  } catch {
    // missing or corrupt config file: fall through to defaults
  }

  cached = { destinationDir: await resolveDefaultDestination() }
  await saveSettings(cached)
  return cached
}

export async function saveSettings(settings: Settings): Promise<void> {
  cached = settings
  await fs.mkdir(path.dirname(configPath()), { recursive: true })
  await fs.writeFile(configPath(), JSON.stringify(settings, null, 2), 'utf-8')
}

export async function setDestinationDir(dir: string): Promise<Settings> {
  const settings = { destinationDir: dir }
  await saveSettings(settings)
  return settings
}
