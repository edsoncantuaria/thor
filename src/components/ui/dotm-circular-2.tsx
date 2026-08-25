import type { CSSProperties } from 'react'

import styles from './DotmCircular2.module.css'

export type DotMatrixColorPreset =
  | 'solid-theme'
  | 'solid-mint'
  | 'grad-sunset'
  | 'grad-ocean'
  | 'grad-neon'
  | 'grad-aurora'
  | 'grad-fire'
  | 'grad-prism'

export type DotmCircular2Props = {
  size?: number
  dotSize?: number
  color?: string
  colorPreset?: DotMatrixColorPreset
  speed?: number
  ariaLabel?: string
  className?: string
  muted?: boolean
  bloom?: boolean
  animated?: boolean
  opacityBase?: number
  opacityMid?: number
  opacityPeak?: number
  cellPadding?: number
  boxSize?: number
  minSize?: number
}

const RING_ORDER = new Map([
  [1, 0],
  [2, 1],
  [3, 2],
  [9, 3],
  [14, 4],
  [19, 5],
  [23, 6],
  [22, 7],
  [21, 8],
  [15, 9],
  [10, 10],
  [5, 11],
])

const PRESET_FILL: Record<DotMatrixColorPreset, string> = {
  'solid-theme': 'var(--status-working)',
  'solid-mint': 'var(--status-working)',
  'grad-sunset': 'linear-gradient(135deg, var(--status-busy), var(--accent))',
  'grad-ocean': 'linear-gradient(135deg, var(--agent-codex), var(--accent))',
  'grad-neon': 'linear-gradient(135deg, var(--status-working), var(--agent-codex))',
  'grad-aurora': 'linear-gradient(135deg, var(--agent-claude), var(--accent), var(--agent-codex))',
  'grad-fire': 'linear-gradient(135deg, var(--status-offline), var(--status-busy))',
  'grad-prism':
    'linear-gradient(135deg, var(--agent-codex), var(--agent-claude), var(--status-working))',
}

export function DotmCircular2({
  size = 36,
  dotSize = 5,
  color = 'var(--status-working)',
  colorPreset = 'solid-theme',
  speed = 1.8,
  ariaLabel = 'Loading',
  className,
  muted = false,
  bloom = false,
  animated = true,
  opacityBase = 0.08,
  opacityMid = 0.4,
  opacityPeak = 0.95,
  cellPadding = 1,
  boxSize,
  minSize,
}: DotmCircular2Props) {
  const fill = colorPreset === 'solid-theme' ? color : PRESET_FILL[colorPreset]
  const slotSize = boxSize ?? size
  const style = {
    width: slotSize,
    height: slotSize,
    minWidth: minSize,
    minHeight: minSize,
    '--dot-size': `${dotSize}px`,
    '--dot-speed': Math.max(0.1, speed),
    '--dot-color': color,
    '--dot-fill': fill,
    '--dot-opacity-base': opacityBase,
    '--dot-opacity-mid': opacityMid,
    '--dot-opacity-peak': opacityPeak,
  } as CSSProperties

  return (
    <span
      className={`${styles.slot} ${bloom ? styles.bloom : ''} ${muted ? styles.muted : ''} ${className ?? ''}`}
      style={style}
      role="status"
      aria-label={ariaLabel}
    >
      <span
        className={styles.matrix}
        aria-hidden="true"
        style={{ width: size, height: size, gap: cellPadding }}
      >
        {Array.from({ length: 25 }, (_, index) => {
          const row = Math.floor(index / 5)
          const col = index % 5
          const outsideMask = (row === 0 || row === 4) && (col === 0 || col === 4)
          if (outsideMask) return <span key={index} />
          const order = RING_ORDER.get(index)
          return (
            <span
              key={index}
              className={`${styles.dot} ${animated && order !== undefined ? styles.ring : ''}`}
              style={
                {
                  '--ring-order': order ?? 0,
                  opacity:
                    order === undefined ? (index === 12 ? opacityMid : opacityBase) : undefined,
                } as CSSProperties
              }
            />
          )
        })}
      </span>
    </span>
  )
}

const CHECK_DOTS = new Set([9, 13, 16, 17, 10])

export function DotmCheck({
  size = 14,
  dotSize = 2,
  color = 'var(--accent)',
  ariaLabel = 'Completed',
  className,
  cellPadding = 1,
}: Pick<
  DotmCircular2Props,
  'size' | 'dotSize' | 'color' | 'ariaLabel' | 'className' | 'cellPadding'
>) {
  const style = {
    width: size,
    height: size,
    '--dot-size': `${dotSize}px`,
    '--dot-color': color,
    '--dot-fill': color,
    '--dot-opacity-base': 1,
  } as CSSProperties

  return (
    <span
      className={`${styles.slot} ${className ?? ''}`}
      style={style}
      role="status"
      aria-label={ariaLabel}
    >
      <span
        className={styles.matrix}
        aria-hidden="true"
        style={{ width: size, height: size, gap: cellPadding }}
      >
        {Array.from({ length: 25 }, (_, index) =>
          CHECK_DOTS.has(index) ? (
            <span key={index} className={styles.dot} />
          ) : (
            <span key={index} />
          ),
        )}
      </span>
    </span>
  )
}
