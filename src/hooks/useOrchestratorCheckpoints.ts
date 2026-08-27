import { useEffect } from 'react'

import { findProjectIdForCwd } from '../lib/projectPaths'
import { listenOrchestratorIsolatedCheckpoint } from '../lib/tauri'
import { useProjectsStore } from '../stores/projectsStore'

/**
 * An isolated `thor_delegate` job commits its work to its own worktree branch on release —
 * nobody is watching that branch unless something surfaces it. This turns each checkpoint with
 * actual committed work into a Todo on the project it belongs to, so it shows up for review.
 */
export function useOrchestratorCheckpoints(): void {
  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | null = null

    void listenOrchestratorIsolatedCheckpoint((payload) => {
      if (cancelled) return
      if (!payload.diffSummary) return

      const { projects, createTodo } = useProjectsStore.getState()
      const projectId = findProjectIdForCwd(projects, payload.cwd)
      if (!projectId) return

      createTodo(
        `Isolated job checkpoint (${payload.branch}): ${payload.spec.slice(0, 60)}`,
        ['orchestrator'],
        projectId,
      )
    }).then((stop) => {
      if (cancelled) stop()
      else unlisten = stop
    })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])
}
