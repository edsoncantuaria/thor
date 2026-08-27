import { invoke } from '@tauri-apps/api/core'

export type ImpeccableStatus = {
  installed: boolean
  hookEnabled: boolean
  ignoreRules: number
  ignoreFiles: number
  ignoreValues: number
}

export async function impeccableStatus(repoPath: string): Promise<ImpeccableStatus> {
  return invoke<ImpeccableStatus>('impeccable_status', { repoPath })
}

export async function impeccableInstall(repoPath: string): Promise<void> {
  return invoke<void>('impeccable_install', { repoPath })
}

export async function impeccableSetHook(repoPath: string, enabled: boolean): Promise<void> {
  return invoke<void>('impeccable_set_hook', { repoPath, enabled })
}
