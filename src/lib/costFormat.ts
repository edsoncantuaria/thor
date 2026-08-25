/** Shared cost and token formatting for TokenHud and Agent Canvas. */

/** Readable USD format: four decimals below $1, two otherwise. */
export function fmtUsd(v: number): string {
  if (v < 1) return `$${v.toFixed(4)}`
  return `$${v.toFixed(2)}`
}

/** Format tokens with k/M suffixes. */
export function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

export type CostLevel = 'low' | 'mid' | 'high'

/** Map spending to a token color level. */
export function costLevel(v: number): CostLevel {
  if (v >= 5) return 'high'
  if (v >= 1) return 'mid'
  return 'low'
}

/** Short model family name for badges. */
export function shortModel(model: string | null): string | null {
  if (!model) return null
  const m = model.toLowerCase()
  if (m.includes('opus')) return 'opus'
  if (m.includes('sonnet')) return 'sonnet'
  if (m.includes('haiku')) return 'haiku'
  return model
}
