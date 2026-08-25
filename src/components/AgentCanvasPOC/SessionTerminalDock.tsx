import { PiggyBank, RotateCcw } from 'lucide-react'

import { PTY_ENV } from '../../lib/agentCanvasConfig'
import { orchestrationRules } from '../../lib/agentCanvasUtils'
import { useT } from '../../lib/i18n'
import type { Theme } from '../../lib/types'
import { XTermView } from '../XTermView'
import styles from './AgentCanvasPOC.module.css'

type SessionTerminalDockProps = {
  session: { folder: string; ptyId: string }
  restartHint: boolean
  claudeExited: number | null
  economyOn: boolean
  onToggleEconomy: () => void
  onRestart: () => void
  hooksSettingsPath: string | null
  hooksEndpoint: string | null
  hooksError: string | null
  onRetryHooks: () => void
  coreAgentsReady: boolean
  terminalTheme: Theme
  budgetUsd: number | null
  onClaudeExit: (code: number | null) => void
}

/** Bottom dock containing the session lead terminal. */
export function SessionTerminalDock({
  session,
  restartHint,
  claudeExited,
  economyOn,
  onToggleEconomy,
  onRestart,
  hooksSettingsPath,
  hooksEndpoint,
  hooksError,
  onRetryHooks,
  coreAgentsReady,
  terminalTheme,
  budgetUsd,
  onClaudeExit,
}: SessionTerminalDockProps) {
  const t = useT()
  return (
    <div className={styles.terminalDock}>
      <div className={styles.terminalHeader}>
        <span className={styles.terminalLabel}>
          claude --dangerously-skip-permissions · teams on
        </span>
        <span className={styles.terminalCwd}>{session.folder}</span>
        {restartHint ? (
          <span className={styles.economyHint}>{t('ws.agentsChangedRestart')}</span>
        ) : null}
        {claudeExited !== null ? (
          <span className={styles.terminalExited}>
            {t('ws.exitedCode', { code: claudeExited })}
          </span>
        ) : null}
        <button
          type="button"
          className={economyOn ? `${styles.clearButton} ${styles.economyOn}` : styles.clearButton}
          onClick={onToggleEconomy}
          title={t('ws.economyModeTitle')}
        >
          <PiggyBank size={14} />
          {t('ws.economy')} {economyOn ? t('ws.on') : t('ws.off')}
        </button>
        <button
          type="button"
          className={styles.clearButton}
          onClick={onRestart}
          title={t('ws.restartClaudeTitle')}
        >
          <RotateCcw size={14} />
        </button>
      </div>
      <div className={styles.terminalHost}>
        {hooksSettingsPath && hooksEndpoint && coreAgentsReady ? (
          <XTermView
            ptyId={session.ptyId}
            command="claude"
            cwd={session.folder}
            extraArgs={[
              '--dangerously-skip-permissions',
              '--settings',
              hooksSettingsPath,
              '--append-system-prompt',
              orchestrationRules(hooksEndpoint, budgetUsd),
            ]}
            env={PTY_ENV}
            terminalTheme={terminalTheme}
            onSpawned={(id) => console.log('[AgentCanvasPOC] claude spawnado, pty:', id)}
            onExit={(code) => {
              console.log('[AgentCanvasPOC] claude saiu, code:', code)
              onClaudeExit(code)
            }}
          />
        ) : hooksError ? (
          <div className={styles.empty}>
            <span>{t('ws.hooksSettingsFailed', { message: hooksError })}</span>
            <button type="button" className={styles.clearButton} onClick={onRetryHooks}>
              {t('errorBoundary.retry')}
            </button>
          </div>
        ) : (
          <div className={styles.empty}>{t('ws.generatingHooksSettings')}</div>
        )}
      </div>
    </div>
  )
}
