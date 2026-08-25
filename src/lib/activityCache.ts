import { getMultiAgentActivity } from './tauri'
import { makeKeyedTtlCache } from './ttlCache'

const TTL_MS = 5 * 60_000

/** Atividade combinada de Claude + Codex + OpenCode (ver get_multi_agent_activity
 * em claude_sessions.rs). Cacheada por `days`. */
export const getCachedActivity = makeKeyedTtlCache(getMultiAgentActivity, TTL_MS)
