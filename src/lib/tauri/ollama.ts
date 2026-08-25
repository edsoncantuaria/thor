import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const PULL_PROGRESS_EVENT = 'ollama-pull-progress'

export type OllamaModelInfo = {
  name: string
  sizeBytes: number
}

export type OllamaInstanceInfo = {
  id: string
  port: number
  model: string
  pid: number
}

export type OllamaPullProgress = {
  model: string
  line: string
  done: boolean
}

type RawOllamaModelInfo = {
  name: string
  size_bytes: number
}

function toOllamaModelInfo(raw: RawOllamaModelInfo): OllamaModelInfo {
  return { name: raw.name, sizeBytes: raw.size_bytes }
}

export async function ollamaIsInstalled(): Promise<boolean> {
  return invoke<boolean>('ollama_is_installed')
}

export async function ollamaInstall(): Promise<void> {
  return invoke<void>('ollama_install')
}

export async function ollamaListModels(port?: number): Promise<OllamaModelInfo[]> {
  const raw = await invoke<RawOllamaModelInfo[]>('ollama_list_models', { port })
  return raw.map(toOllamaModelInfo)
}

export async function ollamaPullModel(model: string): Promise<void> {
  return invoke<void>('ollama_pull_model', { model })
}

export async function ollamaListInstances(): Promise<OllamaInstanceInfo[]> {
  return invoke<OllamaInstanceInfo[]>('ollama_list_instances')
}

export async function ollamaStartInstance(model: string, port?: number): Promise<OllamaInstanceInfo> {
  return invoke<OllamaInstanceInfo>('ollama_start_instance', { model, port })
}

export async function ollamaStopInstance(id: string): Promise<void> {
  return invoke<void>('ollama_stop_instance', { id })
}

export function listenOllamaPullProgress(
  handler: (progress: OllamaPullProgress) => void,
): Promise<UnlistenFn> {
  return listen<OllamaPullProgress>(PULL_PROGRESS_EVENT, (event) => handler(event.payload))
}
