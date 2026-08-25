import { describe, expect, it } from 'vitest'

import { costLevel, fmtTokens, fmtUsd, shortModel } from './costFormat'

describe('fmtUsd', () => {
  it('usa 4 casas abaixo de $1', () => {
    expect(fmtUsd(0)).toBe('$0.0000')
    expect(fmtUsd(0.12345)).toBe('$0.1235') // arredonda
    expect(fmtUsd(0.999)).toBe('$0.9990')
  })

  it('usa 2 casas em $1 ou mais', () => {
    expect(fmtUsd(1)).toBe('$1.00')
    expect(fmtUsd(12.345)).toBe('$12.35')
  })
})

describe('fmtTokens', () => {
  it('mostra número cru abaixo de mil', () => {
    expect(fmtTokens(0)).toBe('0')
    expect(fmtTokens(999)).toBe('999')
  })

  it('usa k a partir de mil', () => {
    expect(fmtTokens(1_000)).toBe('1.0k')
    expect(fmtTokens(1_500)).toBe('1.5k')
    expect(fmtTokens(999_999)).toBe('1000.0k')
  })

  it('usa M a partir de um milhão', () => {
    expect(fmtTokens(1_000_000)).toBe('1.0M')
    expect(fmtTokens(2_500_000)).toBe('2.5M')
  })
})

describe('costLevel', () => {
  it('classifica por faixa de USD (limites inclusivos)', () => {
    expect(costLevel(0)).toBe('low')
    expect(costLevel(0.99)).toBe('low')
    expect(costLevel(1)).toBe('mid')
    expect(costLevel(4.99)).toBe('mid')
    expect(costLevel(5)).toBe('high')
    expect(costLevel(100)).toBe('high')
  })
})

describe('shortModel', () => {
  it('retorna null para model nulo', () => {
    expect(shortModel(null)).toBeNull()
  })

  it('extrai a família (case-insensitive)', () => {
    expect(shortModel('claude-opus-4-8')).toBe('opus')
    expect(shortModel('claude-3-5-SONNET')).toBe('sonnet')
    expect(shortModel('claude-haiku-4-5')).toBe('haiku')
  })

  it('devolve o id original quando não reconhece a família', () => {
    expect(shortModel('gpt-4o')).toBe('gpt-4o')
  })
})
