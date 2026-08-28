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

/** Live stats for one running instance. `proxied: false` means the instance
 * fell back to running Ollama directly (no request path through Thor), so
 * the throughput fields stay at zero and `tokensPerSecond` is `null` — treat
 * those as "not tracked", not as real zero usage. `gpuPercent` is `null`
 * whenever no supported GPU tool (currently `nvidia-smi`) could be found. */
export type OllamaInstanceStats = {
  id: string
  cpuPercent: number
  ramMb: number
  gpuPercent: number | null
  proxied: boolean
  promptTokensTotal: number
  evalTokensTotal: number
  requestsTotal: number
  tokensPerSecond: number | null
}

type RawOllamaModelInfo = {
  name: string
  size_bytes: number
}

type RawOllamaInstanceStats = {
  id: string
  cpu_percent: number
  ram_mb: number
  gpu_percent: number | null
  proxied: boolean
  prompt_tokens_total: number
  eval_tokens_total: number
  requests_total: number
  tokens_per_second: number | null
}

function toOllamaModelInfo(raw: RawOllamaModelInfo): OllamaModelInfo {
  return { name: raw.name, sizeBytes: raw.size_bytes }
}

function toOllamaInstanceStats(raw: RawOllamaInstanceStats): OllamaInstanceStats {
  return {
    id: raw.id,
    cpuPercent: raw.cpu_percent,
    ramMb: raw.ram_mb,
    gpuPercent: raw.gpu_percent,
    proxied: raw.proxied,
    promptTokensTotal: raw.prompt_tokens_total,
    evalTokensTotal: raw.eval_tokens_total,
    requestsTotal: raw.requests_total,
    tokensPerSecond: raw.tokens_per_second,
  }
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

/** A daemon reachable on the standard Ollama port that Thor did not start
 * itself (a system service, or one run by hand) — read-only, Thor has no
 * way to stop or otherwise manage it. `null` when nothing answers there, or
 * when Thor's own managed instance already occupies that port. */
export type ExternalOllamaInfo = {
  port: number
  models: OllamaModelInfo[]
}

type RawExternalOllamaInfo = {
  port: number
  models: RawOllamaModelInfo[]
}

export async function ollamaDetectExternal(): Promise<ExternalOllamaInfo | null> {
  const raw = await invoke<RawExternalOllamaInfo | null>('ollama_detect_external')
  return raw ? { port: raw.port, models: raw.models.map(toOllamaModelInfo) } : null
}

export async function ollamaPullModel(model: string): Promise<void> {
  return invoke<void>('ollama_pull_model', { model })
}

export async function ollamaListInstances(): Promise<OllamaInstanceInfo[]> {
  return invoke<OllamaInstanceInfo[]>('ollama_list_instances')
}

export async function ollamaStartInstance(
  model: string,
  port?: number,
): Promise<OllamaInstanceInfo> {
  return invoke<OllamaInstanceInfo>('ollama_start_instance', { model, port })
}

export async function ollamaStopInstance(id: string): Promise<void> {
  return invoke<void>('ollama_stop_instance', { id })
}

export async function ollamaGetInstanceStats(id: string): Promise<OllamaInstanceStats> {
  const raw = await invoke<RawOllamaInstanceStats>('ollama_get_instance_stats', { id })
  return toOllamaInstanceStats(raw)
}

/** Ensures `opencode.json` (at the repo root for `repo`) has a working "ollama"
 * provider entry before launching an OpenCode terminal with `--model
 * ollama/<model>` — OpenCode doesn't auto-discover local Ollama daemons, it
 * rejects an unregistered provider outright. Never touches other keys
 * (other providers, `mcp`, etc.) in that file. */
export async function ollamaOpenCodeConfigWrite(repo: string, model: string): Promise<void> {
  return invoke('ollama_opencode_config_write', { repo, model })
}

export function listenOllamaPullProgress(
  handler: (progress: OllamaPullProgress) => void,
): Promise<UnlistenFn> {
  return listen<OllamaPullProgress>(PULL_PROGRESS_EVENT, (event) => handler(event.payload))
}
