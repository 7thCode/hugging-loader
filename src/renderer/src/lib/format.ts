export function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '-'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex++
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`
}

export function formatParamCount(paramCount: number | null): string {
  if (paramCount === null) return '不明'
  if (paramCount >= 1_000_000_000) return `${(paramCount / 1_000_000_000).toFixed(1)}B`
  if (paramCount >= 1_000_000) return `${(paramCount / 1_000_000).toFixed(0)}M`
  return String(paramCount)
}
