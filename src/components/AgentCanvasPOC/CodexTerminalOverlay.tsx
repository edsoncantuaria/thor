import { Trash2, X } from 'lucide-react'

import type { CodexWorker } from '../../lib/agentCanvasUtils'
import { useT } from '../../lib/i18n'
import type { Theme } from '../../lib/types'
import { AgentIcon } from '../icons/AgentIcons'
import { XTermView } from '../XTermView'
import styles from './AgentCanvasPOC.module.css'

type CodexTerminalOverlayProps = {
  worker: CodexWorker
  uiTheme: Theme
  terminalTheme: Theme
  onClose: () => void
  onKill: (ptyId: string) => void
  onExit: (ptyId: string, code: number | null) => void
}

/**
 * Terminal codex completo (expandido). Colapsar desmonta o XTermView mas o PTY
 * segue vivo no backend — reabrir re-attacha o scrollback.
 */
export function CodexTerminalOverlay({
  worker: w,
  uiTheme,
  terminalTheme,
  onClose,
  onKill,
  onExit,
}: CodexTerminalOverlayProps) {
  const t = useT()
  return (
    <div className={styles.codexOverlay} onClick={onClose}>
      <div className={styles.codexModal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.terminalHeader}>
          <span className={styles.codexType}>
            <AgentIcon type={w.agent} size={16} theme={uiTheme} /> {t('ws.codexWorker')}
          </span>
          <span className={styles.terminalCwd}>{w.title}</span>
          {w.exitedCode !== null ? (
            <span className={styles.terminalExited}>
              {t('ws.exitedCode', { code: w.exitedCode })}
            </span>
          ) : null}
          <button
            type="button"
            className={styles.clearButton}
            onClick={() => onKill(w.ptyId)}
            title={t('ws.killCodexWorker')}
          >
            <Trash2 size={14} />
          </button>
          <button
            type="button"
            className={styles.clearButton}
            onClick={onClose}
            title={t('ws.closeKeepCodexRunning')}
          >
            <X size={14} />
          </button>
        </div>
        <div className={styles.terminalHost}>
          <XTermView
            ptyId={w.ptyId}
            command={w.agent}
            cwd={w.cwd}
            extraArgs={w.args}
            terminalTheme={terminalTheme}
            onSpawned={(id) => console.log('[AgentCanvasPOC] codex worker spawnado, pty:', id)}
            onExit={(code) => {
              console.log('[AgentCanvasPOC] codex worker saiu, code:', code)
              onExit(w.ptyId, code)
            }}
          />
        </div>
      </div>
    </div>
  )
}
