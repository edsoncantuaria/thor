export const THOR_FILE_DRAG_TYPE = 'application/x-thor-file'

export type ThorFileDragPayload = {
  projectId: string
  path: string
}

export function writeFileDragPayload(
  dataTransfer: DataTransfer,
  payload: ThorFileDragPayload,
): void {
  dataTransfer.effectAllowed = 'copy'
  dataTransfer.setData(THOR_FILE_DRAG_TYPE, JSON.stringify(payload))
  dataTransfer.setData('text/plain', payload.path)
}

export function readFileDragPayload(dataTransfer: DataTransfer): ThorFileDragPayload | null {
  const raw = dataTransfer.getData(THOR_FILE_DRAG_TYPE)
  if (!raw) return null
  try {
    const value = JSON.parse(raw) as Partial<ThorFileDragPayload>
    if (typeof value.projectId !== 'string' || typeof value.path !== 'string') return null
    const projectId = value.projectId.trim()
    const path = value.path.trim()
    return projectId && path ? { projectId, path } : null
  } catch {
    return null
  }
}

export function hasFileDragPayload(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(THOR_FILE_DRAG_TYPE)
}
