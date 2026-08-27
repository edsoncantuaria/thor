import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { OrchestratorBucketConfig } from '../types'

export type OrchestratorJobStatus =
  'queued' | 'running' | 'done' | 'failed' | 'cancelled' | 'released'

export type OrchestratorJob = {
  id: string
  spec: string
  cwd: string
  bucket: string
  model: string | null
  status: OrchestratorJobStatus
  threadId: string | null
  outcome: string | null
  seconds: number | null
  plan: string[]
  tokens: unknown
  hasDiff: boolean
  summary: string
}

export type OrchestratorBucketInfo = {
  id: string
  label: string
  protocol: 'appServer' | 'oneShot'
  defaultModel: string | null
  fallback: string | null
  custom: boolean
}

export type OrchestratorBucketSaveStatus =
  { id: string; resolved: boolean; path: string | null } | { error: string }

export type OrchestratorSnapshot = {
  jobs: OrchestratorJob[]
  running: number
  queued: number
  concurrencyLimit: number
}

const JOBS_EVENT = 'orchestrator://jobs'

export async function orchestratorMcpConfigPath(): Promise<string> {
  return invoke<string>('orchestrator_mcp_config_path')
}

export async function orchestratorJobs(): Promise<OrchestratorSnapshot> {
  return invoke<OrchestratorSnapshot>('orchestrator_jobs')
}

export async function orchestratorSetConcurrency(limit: number): Promise<void> {
  return invoke<void>('orchestrator_set_concurrency', { limit })
}

export async function orchestratorSetBuckets(
  buckets: OrchestratorBucketConfig[],
): Promise<OrchestratorBucketSaveStatus[]> {
  return invoke<OrchestratorBucketSaveStatus[]>('orchestrator_set_buckets', { buckets })
}

export async function orchestratorListBuckets(): Promise<{ buckets: OrchestratorBucketInfo[] }> {
  return invoke<{ buckets: OrchestratorBucketInfo[] }>('orchestrator_list_buckets')
}

export async function listenOrchestratorJobs(
  handler: (jobs: OrchestratorJob[]) => void,
): Promise<UnlistenFn> {
  return listen<{ jobs: OrchestratorJob[] }>(JOBS_EVENT, (event) => {
    handler(event.payload.jobs ?? [])
  })
}
