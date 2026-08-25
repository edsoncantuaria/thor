import { getCodexUsage } from './tauri'
import { makeTtlCache } from './ttlCache'

const TTL_MS = 60_000

export const getCachedCodexUsage = makeTtlCache(getCodexUsage, TTL_MS)
