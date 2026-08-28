import { useEffect, useState } from 'react'

import { useT } from '../../lib/i18n'
import {
  ollamaDetectExternal,
  ollamaGetInstanceStats,
  type ExternalOllamaInfo,
  type OllamaInstanceInfo,
  type OllamaInstanceStats,
  ollamaIsInstalled,
  ollamaListInstances,
  ollamaListModels,
  type OllamaModelInfo,
  ollamaStartInstance,
  ollamaStopInstance,
} from '../../lib/tauri'
import { useUiStore } from '../../stores/uiStore'
import { EmptyState } from '../EmptyState'
import { OllamaIcon } from '../icons/AgentIcons'
import controls from '../modals/controls.module.css'
import styles from './LocalAiView.module.css'

const INSTANCE_POLL_MS = 3000
const STATS_POLL_MS = 2000

function formatMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${Math.round(mb)} MB`
}

function InstanceCard({
  instance,
  onStopped,
}: {
  instance: OllamaInstanceInfo
  onStopped: () => void
}) {
  const t = useT()
  const [stats, setStats] = useState<OllamaInstanceStats | null>(null)
  const [stopping, setStopping] = useState(false)

  useEffect(() => {
    let disposed = false
    const tick = () => {
      void ollamaGetInstanceStats(instance.id)
        .then((next) => {
          if (!disposed) setStats(next)
        })
        .catch(() => {
          if (!disposed) setStats(null)
        })
    }
    tick()
    const timer = window.setInterval(tick, STATS_POLL_MS)
    return () => {
      disposed = true
      window.clearInterval(timer)
    }
  }, [instance.id])

  const stop = async () => {
    setStopping(true)
    try {
      await ollamaStopInstance(instance.id)
      onStopped()
    } finally {
      setStopping(false)
    }
  }

  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <span className={styles.cardTitle}>{instance.model || t('localAi.unnamedModel')}</span>
        <button
          type="button"
          className={controls.btnDanger}
          disabled={stopping}
          onClick={() => void stop()}
        >
          {stopping ? t('localAi.stopping') : t('localAi.stop')}
        </button>
      </div>
      <div className={styles.cardMeta}>
        {t('localAi.port', { port: instance.port })} · {t('localAi.pid', { pid: instance.pid })}
      </div>
      <div className={styles.statsGrid}>
        <div className={styles.stat}>
          <span className={styles.statLabel}>{t('localAi.cpu')}</span>
          <span className={styles.statValue}>
            {stats ? `${Math.round(stats.cpuPercent)}%` : '—'}
          </span>
        </div>
        <div className={styles.stat}>
          <span className={styles.statLabel}>{t('localAi.ram')}</span>
          <span className={styles.statValue}>{stats ? formatMb(stats.ramMb) : '—'}</span>
        </div>
        {stats?.gpuPercent != null ? (
          <div className={styles.stat}>
            <span className={styles.statLabel}>{t('localAi.gpu')}</span>
            <span className={styles.statValue}>{Math.round(stats.gpuPercent)}%</span>
          </div>
        ) : null}
        {stats?.proxied ? (
          <>
            <div className={styles.stat}>
              <span className={styles.statLabel}>{t('localAi.throughput')}</span>
              <span className={styles.statValue}>
                {stats.tokensPerSecond != null
                  ? t('localAi.tokensPerSecond', { value: stats.tokensPerSecond.toFixed(1) })
                  : '—'}
              </span>
            </div>
            <div className={styles.stat}>
              <span className={styles.statLabel}>{t('localAi.requests')}</span>
              <span className={styles.statValue}>{stats.requestsTotal}</span>
            </div>
          </>
        ) : (
          <p className={styles.statNote}>{t('localAi.throughputUnavailable')}</p>
        )}
      </div>
    </div>
  )
}

function ExternalOllamaCard({ info }: { info: ExternalOllamaInfo }) {
  const t = useT()
  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <span className={styles.cardTitle}>{t('localAi.externalTitle')}</span>
        <span className={styles.externalBadge}>{t('localAi.externalBadge')}</span>
      </div>
      <div className={styles.cardMeta}>{t('localAi.port', { port: info.port })}</div>
      <p className={styles.statNote}>{t('localAi.externalDesc')}</p>
      <ul className={styles.externalModelList}>
        {info.models.map((model) => (
          <li key={model.name}>{model.name}</li>
        ))}
      </ul>
    </div>
  )
}

export function LocalAiView() {
  const t = useT()
  const openModal = useUiStore((state) => state.openModal_)
  const [installed, setInstalled] = useState<boolean | null>(null)
  const [models, setModels] = useState<OllamaModelInfo[]>([])
  const [instances, setInstances] = useState<OllamaInstanceInfo[]>([])
  const [externalOllama, setExternalOllama] = useState<ExternalOllamaInfo | null>(null)
  const [selectedModel, setSelectedModel] = useState('')
  const [starting, setStarting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refreshInstances = () => {
    void ollamaListInstances()
      .then(setInstances)
      .catch(() => setInstances([]))
  }

  const refreshExternal = () => {
    void ollamaDetectExternal()
      .then(setExternalOllama)
      .catch(() => setExternalOllama(null))
  }

  useEffect(() => {
    let disposed = false
    void ollamaIsInstalled().then((value) => {
      if (disposed) return
      setInstalled(value)
      if (value) {
        void ollamaListModels()
          .then(setModels)
          .catch(() => setModels([]))
        refreshInstances()
      }
    })
    return () => {
      disposed = true
    }
  }, [])

  useEffect(() => {
    if (!installed) return
    const timer = window.setInterval(refreshInstances, INSTANCE_POLL_MS)
    return () => window.clearInterval(timer)
  }, [installed])

  useEffect(() => {
    refreshExternal()
    const timer = window.setInterval(refreshExternal, INSTANCE_POLL_MS)
    return () => window.clearInterval(timer)
  }, [])

  const startInstance = async () => {
    if (!selectedModel) return
    setStarting(true)
    setError(null)
    try {
      await ollamaStartInstance(selectedModel)
      refreshInstances()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setStarting(false)
    }
  }

  return (
    <div className={styles.root}>
      <header className={styles.header}>
        <div className={styles.headerIcon}>
          <OllamaIcon size={20} />
        </div>
        <div>
          <h1 className={styles.title}>{t('localAi.title')}</h1>
          <p className={styles.subtitle}>{t('localAi.subtitle')}</p>
        </div>
      </header>

      {installed === false ? (
        <EmptyState
          icon={<OllamaIcon size={28} />}
          title={t('localAi.notInstalledTitle')}
          description={t('localAi.notInstalledDesc')}
          primaryAction={{
            label: t('localAi.openPreferences'),
            onClick: () => openModal('preferences'),
          }}
        />
      ) : null}

      {installed ? (
        <>
          <div className={styles.startRow}>
            <select
              className={`${controls.input} ${styles.select}`}
              value={selectedModel}
              onChange={(event) => setSelectedModel(event.target.value)}
            >
              <option value="">{t('localAi.selectModel')}</option>
              {models.map((model) => (
                <option key={model.name} value={model.name}>
                  {model.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              className={`${controls.btn} ${controls.btnPrimary}`}
              disabled={!selectedModel || starting}
              onClick={() => void startInstance()}
            >
              {starting ? t('localAi.starting') : t('localAi.start')}
            </button>
          </div>

          {error ? <p className={styles.error}>{error}</p> : null}

          {instances.length === 0 && !externalOllama ? (
            <EmptyState
              icon={<OllamaIcon size={28} />}
              title={t('localAi.noInstancesTitle')}
              description={t('localAi.noInstancesDesc')}
              compact
            />
          ) : (
            <div className={styles.grid}>
              {externalOllama ? <ExternalOllamaCard info={externalOllama} /> : null}
              {instances.map((instance) => (
                <InstanceCard key={instance.id} instance={instance} onStopped={refreshInstances} />
              ))}
            </div>
          )}
        </>
      ) : externalOllama ? (
        <div className={styles.grid}>
          <ExternalOllamaCard info={externalOllama} />
        </div>
      ) : null}
    </div>
  )
}
