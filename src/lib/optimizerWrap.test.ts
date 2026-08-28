import { describe, expect, it } from 'vitest'

import { applyOptimizerWrap } from './optimizerWrap'

describe('applyOptimizerWrap', () => {
  it('leaves the command untouched when the wrapper is none', () => {
    expect(applyOptimizerWrap('none', 'claude', ['--resume'])).toEqual({
      command: 'claude',
      extraArgs: ['--resume'],
    })
  })

  it('leaves plain shell panes (no command) untouched regardless of wrapper', () => {
    expect(applyOptimizerWrap('caveman', undefined, undefined)).toEqual({
      command: undefined,
      extraArgs: undefined,
    })
  })

  it('prefixes caveman with the agent name as the first argument', () => {
    expect(applyOptimizerWrap('caveman', 'claude', ['--resume'])).toEqual({
      command: 'caveman',
      extraArgs: ['claude', '--resume'],
    })
  })

  it('prefixes headroom with "wrap" and the agent name', () => {
    expect(applyOptimizerWrap('headroom', 'codex', [])).toEqual({
      command: 'headroom',
      extraArgs: ['wrap', 'codex'],
    })
  })

  it('wraps caveman with "run --" for agents caveman does not recognize (e.g. Antigravity)', () => {
    expect(applyOptimizerWrap('caveman', 'agy', ['--dangerously-skip-permissions'])).toEqual({
      command: 'caveman',
      extraArgs: ['run', '--', 'agy', '--dangerously-skip-permissions'],
    })
  })

  it('defaults extraArgs to an empty array when none were passed', () => {
    expect(applyOptimizerWrap('caveman', 'claude', undefined)).toEqual({
      command: 'caveman',
      extraArgs: ['claude'],
    })
  })
})
