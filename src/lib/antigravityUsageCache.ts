import { getAntigravityUsage } from './tauri'
import { makeTtlCache } from './ttlCache'

const TTL_MS = 60_000

export const getCachedAntigravityUsage = makeTtlCache(getAntigravityUsage, TTL_MS)
