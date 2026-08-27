import { describe, expect, it } from 'vitest'

import { findProjectIdForCwd } from './projectPaths'
import type { Project } from './types'

function project(id: string, defaultCwd?: string): Project {
  return {
    id,
    name: id,
    groupId: null,
    terminals: [],
    layoutMode: 'auto',
    collapsed: false,
    createdAt: 0,
    ...(defaultCwd ? { defaultCwd } : {}),
  }
}

describe('findProjectIdForCwd', () => {
  it('matches a project whose defaultCwd equals cwd exactly', () => {
    const projects = [project('a', '/home/u/other'), project('b', '/home/u/app')]
    expect(findProjectIdForCwd(projects, '/home/u/app')).toBe('b')
  })

  it('matches a project whose defaultCwd is an ancestor of cwd', () => {
    const projects = [project('a', '/home/u/other'), project('b', '/home/u/app')]
    expect(findProjectIdForCwd(projects, '/home/u/app/src/worker')).toBe('b')
  })

  it('returns null when no project matches', () => {
    const projects = [project('a', '/home/u/other')]
    expect(findProjectIdForCwd(projects, '/home/u/app')).toBeNull()
  })

  it('skips projects with no defaultCwd', () => {
    const projects = [project('no-cwd'), project('b', '/home/u/app')]
    expect(findProjectIdForCwd(projects, '/home/u/app')).toBe('b')
    expect(findProjectIdForCwd([project('no-cwd')], '/home/u/app')).toBeNull()
  })

  it('normalizes Windows-style path separators and drive letter case', () => {
    const projects = [project('b', 'C:\\dev\\thor')]
    expect(findProjectIdForCwd(projects, 'c:/dev/thor')).toBe('b')
    expect(findProjectIdForCwd(projects, 'c:/dev/thor/src')).toBe('b')
  })
})
