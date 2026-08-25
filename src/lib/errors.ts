/** Shared error helpers for normalizing Tauri rejection messages. */

/** Return a readable message for any thrown value. */
export function messageOf(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

/** Remove the backend `code:` prefix from a rejection. */
export function readableError(error: unknown): string {
  const value = messageOf(error)
  const separator = value.indexOf(':')
  return separator >= 0 ? value.slice(separator + 1).trim() : value
}

/** No-op for intentionally ignored promise failures. */
export function ignore(): void {}
