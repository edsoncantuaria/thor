import { invoke } from '@tauri-apps/api/core'

export async function optimizerInstallCaveman(): Promise<void> {
  return invoke<void>('optimizer_install_caveman')
}

export async function optimizerInstallRtk(): Promise<void> {
  return invoke<void>('optimizer_install_rtk')
}

export async function optimizerConfigureRtk(): Promise<void> {
  return invoke<void>('optimizer_configure_rtk')
}

export async function optimizerInstallHeadroom(): Promise<void> {
  return invoke<void>('optimizer_install_headroom')
}
