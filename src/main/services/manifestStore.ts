import { promises as fs } from 'fs'
import path from 'path'

// A small metadata file written into the destination directory itself, so the
// download history travels with the models folder (survives app reinstall,
// is inspectable by hand) and disambiguates same-named files from different repos.
export const MANIFEST_FILENAME = '.hugging-loader-manifest.json'

export interface ManifestEntry {
  repoId: string
  quant: string | null
  sizeBytes: number
  paramCount: number | null
  downloadedAt: string // ISO timestamp
}

export type Manifest = Record<string, ManifestEntry> // keyed by filename

function manifestPath(destinationDir: string): string {
  return path.join(destinationDir, MANIFEST_FILENAME)
}

export async function loadManifest(destinationDir: string): Promise<Manifest> {
  try {
    const raw = await fs.readFile(manifestPath(destinationDir), 'utf-8')
    const parsed = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? (parsed as Manifest) : {}
  } catch {
    return {}
  }
}

async function saveManifest(destinationDir: string, manifest: Manifest): Promise<void> {
  await fs.writeFile(manifestPath(destinationDir), JSON.stringify(manifest, null, 2), 'utf-8')
}

export async function recordDownload(
  destinationDir: string,
  filename: string,
  entry: ManifestEntry
): Promise<void> {
  const manifest = await loadManifest(destinationDir)
  manifest[filename] = entry
  await saveManifest(destinationDir, manifest)
}

export async function removeManifestEntry(destinationDir: string, filename: string): Promise<void> {
  const manifest = await loadManifest(destinationDir)
  if (filename in manifest) {
    delete manifest[filename]
    await saveManifest(destinationDir, manifest)
  }
}
