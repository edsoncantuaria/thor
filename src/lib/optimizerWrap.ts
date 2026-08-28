export type OptimizerWrapper = 'none' | 'caveman' | 'headroom'

export type OptimizerWrapResult = {
  command: string | undefined
  extraArgs: string[] | undefined
}

// Agent CLIs caveman recognizes by name (`caveman <agent>`). Anything else
// must go through `caveman run -- <cmd>` or caveman rejects it with
// "not a known agent". Source: `caveman --help`.
const CAVEMAN_KNOWN_AGENTS = new Set([
  'aider',
  'claude',
  'codex',
  'gemini',
  'hermes',
  'openclaw',
  'opencode',
  'pi',
])

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
    if (CAVEMAN_KNOWN_AGENTS.has(command)) {
      return { command: 'caveman', extraArgs: [command, ...args] }
    }
    return { command: 'caveman', extraArgs: ['run', '--', command, ...args] }
  }
  return { command: 'headroom', extraArgs: ['wrap', command, ...args] }
}
