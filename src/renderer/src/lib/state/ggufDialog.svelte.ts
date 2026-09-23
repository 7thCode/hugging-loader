import type { GgufHeaderResponse } from '../../../../shared/ipc-types'

export class GgufDialogState {
  filename = $state<string | null>(null)
  loading = $state(false)
  error = $state<string | null>(null)
  // A model can carry thousands of tensors; the data is read-only, so skip deep reactivity.
  header = $state.raw<GgufHeaderResponse | null>(null)
}

export const ggufDialog = new GgufDialogState()

// Incremented per open so a slow read for a previously opened file can't overwrite a newer one.
let requestSeq = 0

export async function openGgufDialog(filename: string): Promise<void> {
  const seq = ++requestSeq
  ggufDialog.filename = filename
  ggufDialog.loading = true
  ggufDialog.error = null
  ggufDialog.header = null

  try {
    const header = await window.api.readGgufHeader({ filename })
    if (seq !== requestSeq) return
    ggufDialog.header = header
  } catch (err) {
    if (seq !== requestSeq) return
    ggufDialog.error = err instanceof Error ? err.message : String(err)
  } finally {
    if (seq === requestSeq) ggufDialog.loading = false
  }
}

export function closeGgufDialog(): void {
  requestSeq++
  ggufDialog.filename = null
  ggufDialog.loading = false
  ggufDialog.error = null
  ggufDialog.header = null
}
