export class SettingsState {
  destinationDir = $state('')
  loaded = $state(false)
}

export const settingsState = new SettingsState()

export async function loadSettings(): Promise<void> {
  const s = await window.api.getSettings()
  settingsState.destinationDir = s.destinationDir
  settingsState.loaded = true
}

export async function changeDestinationFolder(): Promise<void> {
  const picked = await window.api.chooseFolder()
  if (picked.canceled || !picked.path) return
  const updated = await window.api.setDestinationDir({ dir: picked.path })
  settingsState.destinationDir = updated.destinationDir
}
