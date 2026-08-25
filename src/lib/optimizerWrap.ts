export type OptimizerWrapper = 'none' | 'caveman' | 'headroom'

export type OptimizerWrapResult = {
  command: string | undefined
  extraArgs: string[] | undefined
}

/**
 * Prefixes an agent's spawn command with the chosen token-optimizer wrapper.
 * `command` is only ever set when spawning a known agent CLI (plain shell
 * panes omit it), so any wrapper here is safe to apply unconditionally.
 */
export function applyOptimizerWrap(
  wrapper: OptimizerWrapper,
  command: string | undefined,
  extraArgs: string[] | undefined,
): OptimizerWrapResult {
  if (!command || wrapper === 'none') {
    return { command, extraArgs }
  }

  const args = extraArgs ?? []
  if (wrapper === 'caveman') {
    return { command: 'caveman', extraArgs: [command, ...args] }
  }
  return { command: 'headroom', extraArgs: ['wrap', command, ...args] }
}
