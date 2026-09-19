import { promises as fs } from 'fs'
import path from 'path'

// `filename` throughout the app is a Hugging Face rfilename: a path relative to the
// destination folder that may include subfolders (e.g. "BF16/model-00001-of-00002.gguf").
// The subfolder layout is preserved on disk so the shards of a split GGUF stay together
// (llama.cpp needs them in one directory) and identically-named files from different
// folders of a repo can't collide. It also comes from the renderer, so it must never be
// able to point outside the destination folder (e.g. "../../.ssh/id_rsa").
export function resolveInDestination(destinationDir: string, filename: string): string {
  const root = path.resolve(destinationDir)
  const resolved = path.resolve(root, filename)
  if (!resolved.startsWith(root + path.sep)) {
    throw new Error('保存先フォルダ外のパスは指定できません')
  }
  return resolved
}

// Stats each candidate instead of listing the destination: files live in subfolders, and a
// recursive walk of a large models folder would be far more work than one stat per file.
export async function findExistingFilenames(
  destinationDir: string,
  filenames: string[]
): Promise<Set<string>> {
  const found = await Promise.all(
    filenames.map(async (filename) => {
      try {
        const stat = await fs.stat(resolveInDestination(destinationDir, filename))
        return stat.isFile() ? filename : null
      } catch {
        return null
      }
    })
  )
  return new Set(found.filter((f): f is string => f !== null))
}

export async function checkExists(
  destinationDir: string,
  filename: string
): Promise<{ exists: boolean; sizeBytes: number | null; path: string }> {
  const filePath = resolveInDestination(destinationDir, filename)
  try {
    const stat = await fs.stat(filePath)
    return { exists: stat.isFile(), sizeBytes: stat.isFile() ? stat.size : null, path: filePath }
  } catch {
    return { exists: false, sizeBytes: null, path: filePath }
  }
}

export async function deleteFile(destinationDir: string, filename: string): Promise<boolean> {
  try {
    const filePath = resolveInDestination(destinationDir, filename)
    await fs.unlink(filePath)
    await pruneEmptyParents(destinationDir, path.dirname(filePath))
    return true
  } catch {
    return false
  }
}

// Downloading into a subfolder creates it, so deleting the last file in it removes it again.
// rmdir refuses non-empty directories, so a folder still holding other shards, a `.part`, or
// anything the user put there is left alone; the destination folder itself is never removed.
async function pruneEmptyParents(destinationDir: string, dir: string): Promise<void> {
  const root = path.resolve(destinationDir)
  let current = dir
  while (current.startsWith(root + path.sep)) {
    try {
      await fs.rmdir(current)
    } catch {
      return
    }
    current = path.dirname(current)
  }
}
