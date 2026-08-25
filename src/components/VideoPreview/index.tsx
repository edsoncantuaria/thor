import { convertFileSrc } from '@tauri-apps/api/core'

import styles from './VideoPreview.module.css'

export function VideoPreview({ path, className }: { path: string; className?: string }) {
  return (
    <video
      className={`${styles.video} ${className ?? ''}`}
      controls
      preload="metadata"
      src={convertFileSrc(path)}
    >
      <track kind="captions" />
    </video>
  )
}
