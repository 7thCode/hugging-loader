import { QUANT_TYPES } from '../../shared/ipc-types'

// Longest-token-first alternation so e.g. Q5_K_M isn't matched as Q5_K's prefix stopping early.
const QUANT_REGEX = new RegExp(
  `\\b(${[...QUANT_TYPES].sort((a, b) => b.length - a.length).join('|')})\\b`,
  'i'
)

export function parseQuant(filename: string): string | null {
  const match = filename.match(QUANT_REGEX)
  return match ? match[1].toUpperCase() : null
}
