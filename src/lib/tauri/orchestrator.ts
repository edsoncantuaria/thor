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
  tokensUsed: number
  tokenBudget: number | null
}

export type OrchestratorIsolatedCheckpoint = {
  jobId: string
  cwd: string
  branch: string
  path: string
  spec: string
  diffSummary: string
}

const JOBS_EVENT = 'orchestrator://jobs'
const ISOLATED_CHECKPOINT_EVENT = 'orchestrator://isolated-checkpoint'

export async function orchestratorMcpConfigPath(): Promise<string> {
  return invoke<string>('orchestrator_mcp_config_path')
}

export async function orchestratorJobs(): Promise<OrchestratorSnapshot> {
  return invoke<OrchestratorSnapshot>('orchestrator_jobs')
}

export async function orchestratorSetConcurrency(limit: number): Promise<void> {
  return invoke<void>('orchestrator_set_concurrency', { limit })
}

export async function orchestratorSetJobTimeoutSecs(secs: number): Promise<void> {
  return invoke<void>('orchestrator_set_job_timeout_secs', { secs })
}

/** `null` clears the cap (unlimited). */
export async function orchestratorSetTokenBudget(budget: number | null): Promise<void> {
  return invoke<void>('orchestrator_set_token_budget', { budget })
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

export async function listenOrchestratorIsolatedCheckpoint(
  handler: (payload: OrchestratorIsolatedCheckpoint) => void,
): Promise<UnlistenFn> {
  return listen<OrchestratorIsolatedCheckpoint>(ISOLATED_CHECKPOINT_EVENT, (event) => {
    handler(event.payload)
  })
}
