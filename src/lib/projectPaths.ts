import { normalizeCwd, pathSegments } from './paths'
import type { Project } from './types'

/**
 * Resolves the project whose `defaultCwd` matches `cwd` exactly, or whose `defaultCwd` is an
 * ancestor directory of `cwd`. Exact match is the common case (`cwd` is normally a project's own
 * root), but the subpath case is handled too for robustness. Projects without a `defaultCwd` are
 * skipped.
 */
export function findProjectIdForCwd(projects: Project[], cwd: string): string | null {
  const cwdSegments = pathSegments(normalizeCwd(cwd))

  for (const project of projects) {
    if (!project.defaultCwd) continue
    const rootSegments = pathSegments(normalizeCwd(project.defaultCwd))
    if (
      rootSegments.length > 0 &&
      rootSegments.length <= cwdSegments.length &&
      rootSegments.every((segment, index) => segment === cwdSegments[index])
    ) {
      return project.id
    }
  }

  return null
}
