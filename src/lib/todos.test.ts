import { describe, expect, it } from 'vitest'

import { normalizeTodoTitle, reorderTodoItems } from './todos'
import type { TodoItem } from './types'

const items: TodoItem[] = [
  { id: 'a', title: 'A', completed: false, tags: [] },
  { id: 'b', title: 'B', completed: false, tags: [] },
  { id: 'c', title: 'C', completed: true, tags: [] },
]

describe('todos', () => {
  it('normalizes titles', () => {
    expect(normalizeTodoTitle('  ship it  ')).toBe('ship it')
    expect(normalizeTodoTitle(null)).toBe('')
  })

  it('reorders tasks inside the same section', () => {
    expect(reorderTodoItems(items, 'b', 'a').map((item) => item.id)).toEqual(['b', 'a', 'c'])
  })

  it('does not move tasks across completion sections', () => {
    expect(reorderTodoItems(items, 'a', 'c')).toBe(items)
  })
})
