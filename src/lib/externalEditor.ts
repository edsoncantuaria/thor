import type { TFunction } from './i18n'
import { findCliLauncher, findEditorLauncher, openInEditor } from './tauri'
import type { Preferences } from './types'

export type ExternalEditorConfig = Pick<Preferences, 'externalEditor' | 'externalEditorCommand'>

/** Human-readable label for the currently configured external editor. */
export function externalEditorLabel(config: ExternalEditorConfig, t: TFunction): string {
  switch (config.externalEditor) {
    case 'vscode':
      return t('prefs.externalEditorVscode')
    case 'cursor':
      return t('prefs.externalEditorCursor')
    case 'custom':
      return config.externalEditorCommand.trim() || t('prefs.externalEditorFallbackLabel')
  }
}

/** Opens `path` in the editor configured in preferences. */
export async function openInConfiguredEditor(
  path: string,
  config: ExternalEditorConfig,
): Promise<void> {
  await openInEditor(path, config.externalEditor, config.externalEditorCommand)
}

/** Resolves whether the configured editor can currently be launched. */
export async function isConfiguredEditorAvailable(config: ExternalEditorConfig): Promise<boolean> {
  if (config.externalEditor === 'custom') {
    const command = config.externalEditorCommand.trim()
    if (!command) return false
    return (await findCliLauncher(command)) !== null
  }
  return (await findEditorLauncher(config.externalEditor)) !== null
}
