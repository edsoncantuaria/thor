const LOCAL_ADDRESS = /^(localhost|127(?:\.\d{1,3}){3}|\[::1\])(?::\d+)?(?:\/|$)/i

/** Normalize typed addresses and restrict the viewer to HTTP(S). */
export function normalizeBrowserUrl(value: string): string | null {
  const input = value.trim()
  if (!input) return null

  const hasScheme = /^https?:\/\//i.test(input)
  const isLocal = LOCAL_ADDRESS.test(input)
  if (!hasScheme && !isLocal && /^[a-z][a-z\d+.-]*:/i.test(input)) return null
  const candidate = hasScheme ? input : `${isLocal ? 'http' : 'https'}://${input}`

  try {
    const url = new URL(candidate)
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.toString() : null
  } catch {
    return null
  }
}
