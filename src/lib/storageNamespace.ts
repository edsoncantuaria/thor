const STORAGE_PREFIX = 'thor'
const LEGACY_PREFIXES = ['alethe', 'ensemble']

let activeNamespace = 'default'

export function setStorageNamespace(namespace: string): void {
  activeNamespace = namespace.trim() || 'default'
}

export function getStorageNamespace(): string {
  return activeNamespace
}

export function scopedStorageKey(key: string): string {
  return `${STORAGE_PREFIX}:${activeNamespace}:${key}`
}

function unscopedStorageKey(prefix: string, key: string): string {
  return `${prefix}:${key}`
}

export function readScopedStorage(key: string, allowLegacy = false): string | null {
  const namespacedKey = scopedStorageKey(key)
  const current = localStorage.getItem(namespacedKey)
  if (current !== null) return current
  if (!allowLegacy || activeNamespace !== 'default') return null

  const candidates = [
    unscopedStorageKey(STORAGE_PREFIX, key),
    ...LEGACY_PREFIXES.flatMap((prefix) => [
      `${prefix}:${activeNamespace}:${key}`,
      unscopedStorageKey(prefix, key),
    ]),
  ]
  for (const legacyKey of candidates) {
    const raw = localStorage.getItem(legacyKey)
    if (raw !== null) {
      localStorage.setItem(namespacedKey, raw)
      return raw
    }
  }
  return null
}

export function writeScopedStorage(key: string, value: string): void {
  localStorage.setItem(scopedStorageKey(key), value)
}

export function removeScopedStorage(key: string): void {
  localStorage.removeItem(scopedStorageKey(key))
}
