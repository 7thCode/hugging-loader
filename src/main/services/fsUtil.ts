import { promises as fs } from 'fs'
import path from 'path'

export async function listExistingFilenames(destinationDir: string): Promise<Set<string>> {
  try {
    const entries = await fs.readdir(destinationDir, { withFileTypes: true })
    return new Set(entries.filter((e) => e.isFile()).map((e) => e.name))
  } catch {
    return new Set()
  }
}

export async function checkExists(
  destinationDir: string,
  filename: string
): Promise<{ exists: boolean; sizeBytes: number | null; path: string }> {
  const filePath = path.join(destinationDir, filename)
  try {
    const stat = await fs.stat(filePath)
    return { exists: stat.isFile(), sizeBytes: stat.isFile() ? stat.size : null, path: filePath }
  } catch {
    return { exists: false, sizeBytes: null, path: filePath }
  }
}

export async function deleteFile(destinationDir: string, filename: string): Promise<boolean> {
  const filePath = path.join(destinationDir, filename)
  try {
    await fs.unlink(filePath)
    return true
  } catch {
    return false
  }
}
