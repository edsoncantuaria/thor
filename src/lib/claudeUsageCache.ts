import { getClaudeUsage } from './tauri'
import { makeTtlCache } from './ttlCache'

const TTL_MS = 60_000

export const getCachedClaudeUsage = makeTtlCache(getClaudeUsage, TTL_MS)
