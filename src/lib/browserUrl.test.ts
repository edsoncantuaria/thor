import { describe, expect, it } from 'vitest'

import { normalizeBrowserUrl } from './browserUrl'

describe('normalizeBrowserUrl', () => {
  it('defaults public hosts to https', () => {
    expect(normalizeBrowserUrl('example.com/docs')).toBe('https://example.com/docs')
  })

  it('uses http for local development addresses', () => {
    expect(normalizeBrowserUrl('localhost:1422')).toBe('http://localhost:1422/')
    expect(normalizeBrowserUrl('127.0.0.1:3000/app')).toBe('http://127.0.0.1:3000/app')
  })

  it('rejects non-http protocols and invalid input', () => {
    expect(normalizeBrowserUrl('javascript:alert(1)')).toBeNull()
    expect(normalizeBrowserUrl('file:///tmp/readme.html')).toBeNull()
    expect(normalizeBrowserUrl('')).toBeNull()
  })
})
