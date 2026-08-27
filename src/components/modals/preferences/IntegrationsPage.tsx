import { nanoid } from 'nanoid'
import { useEffect, useState } from 'react'

import { useT } from '../../../lib/i18n'
import {
  cliShimInstall,
  type CliShimStatus,
  cliShimStatus,
  cliShimUninstall,
  findCliLauncher,
  listenOllamaPullProgress,
  type OllamaInstanceInfo,
  ollamaInstall,
  ollamaIsInstalled,
  ollamaListInstances,
  ollamaListModels,
  type OllamaModelInfo,
  ollamaPullModel,
  ollamaStartInstance,
  ollamaStopInstance,
  optimizerConfigureRtk,
  optimizerInstallCaveman,
  optimizerInstallHeadroom,
  optimizerInstallRtk,
} from '../../../lib/tauri'
import type { OrchestratorBucketConfig, Preferences } from '../../../lib/types'
import { useProjectsStore } from '../../../stores/projectsStore'
import controls from '../controls.module.css'
import styles from '../PreferencesModal.module.css'
import { SettingsSection } from './primitives'

function TerminalCommandSection() {
  const t = useT()
  const [status, setStatus] = useState<CliShimStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let disposed = false
    void cliShimStatus()
      .then((next) => {
        if (!disposed) setStatus(next)
      })
      .catch(() => {
        if (!disposed) setStatus(null)
      })
    return () => {
      disposed = true
    }
  }, [])

  const run = async (action: () => Promise<CliShimStatus>) => {
    setBusy(true)
    setError(null)
    try {
      setStatus(await action())
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(false)
    }
  }

  return (
    <SettingsSection
      id="terminal-command"
      title={t('prefs.cliCommand')}
      description={t('prefs.cliCommandDesc')}
    >
      <div className={styles.integrationFields}>
        <pre className={styles.cliUsage}>
          <code>{'thor\nthor .\nthor ~/meu-projeto'}</code>
        </pre>

        {status?.supported === false ? (
          <p>{t('prefs.cliUnsupported')}</p>
        ) : (
          <>
            <div className={styles.cliActions}>
              <button
                type="button"
                className={`${controls.btn} ${controls.btnPrimary}`}
                disabled={busy || !status}
                onClick={() => void run(cliShimInstall)}
              >
                {status?.installed ? t('prefs.cliReinstall') : t('prefs.cliInstall')}
              </button>
              {status?.installed ? (
                <button
                  type="button"
                  className={`${controls.btn} ${controls.btnDanger}`}
                  disabled={busy}
                  onClick={() => void run(cliShimUninstall)}
                >
                  {t('prefs.cliUninstall')}
                </button>
              ) : null}
            </div>

            {status?.installed && status.path ? (
              <p className={styles.cliPath}>{t('prefs.cliInstalledAt', { path: status.path })}</p>
            ) : null}

            {status?.stale ? <p className={styles.cliWarning}>{t('prefs.cliStale')}</p> : null}

            {status?.installed && !status.onPath && status.binDir ? (
              <p className={styles.cliWarning}>{t('prefs.cliNotOnPath', { dir: status.binDir })}</p>
            ) : null}

            {error ? <p className={styles.cliWarning}>{error}</p> : null}
          </>
        )}
      </div>
    </SettingsSection>
  )
}

function OllamaSection() {
  const t = useT()
  const [installed, setInstalled] = useState<boolean | null>(null)
  const [installing, setInstalling] = useState(false)
  const [models, setModels] = useState<OllamaModelInfo[]>([])
  const [instances, setInstances] = useState<OllamaInstanceInfo[]>([])
  const [pullTarget, setPullTarget] = useState('')
  const [pullingModel, setPullingModel] = useState<string | null>(null)
  const [startModel, setStartModel] = useState('')
  const [error, setError] = useState<string | null>(null)

  const refreshModels = () => {
    void ollamaListModels()
      .then(setModels)
      .catch(() => setModels([]))
  }

  const refreshInstances = () => {
    void ollamaListInstances()
      .then(setInstances)
      .catch(() => setInstances([]))
  }

  useEffect(() => {
    let disposed = false
    void ollamaIsInstalled().then((value) => {
      if (disposed) return
      setInstalled(value)
      if (value) {
        refreshModels()
        refreshInstances()
      }
    })
    return () => {
      disposed = true
    }
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    void listenOllamaPullProgress((progress) => {
      if (progress.done) {
        setPullingModel(null)
        refreshModels()
      }
    }).then((fn) => {
      unlisten = fn
    })
    return () => unlisten?.()
  }, [])

  const install = async () => {
    setInstalling(true)
    setError(null)
    try {
      await ollamaInstall()
      setInstalled(true)
      refreshModels()
      refreshInstances()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setInstalling(false)
    }
  }

  const pull = async () => {
    const model = pullTarget.trim()
    if (!model) return
    setError(null)
    setPullingModel(model)
    try {
      await ollamaPullModel(model)
      setPullTarget('')
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      setPullingModel(null)
    }
  }

  const startInstance = async () => {
    if (!startModel) return
    setError(null)
    try {
      await ollamaStartInstance(startModel)
      refreshInstances()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    }
  }

  const stopInstance = async (id: string) => {
    setError(null)
    try {
      await ollamaStopInstance(id)
      refreshInstances()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    }
  }

  return (
    <SettingsSection id="ollama" title={t('prefs.ollama')} description={t('prefs.ollamaDesc')}>
      <div className={styles.integrationFields}>
        {installed === false ? (
          <div className={styles.cliActions}>
            <p>{t('prefs.ollamaNotInstalled')}</p>
            <button
              type="button"
              className={`${controls.btn} ${controls.btnPrimary}`}
              disabled={installing}
              onClick={() => void install()}
            >
              {installing ? t('prefs.ollamaInstalling') : t('prefs.ollamaInstall')}
            </button>
          </div>
        ) : null}

        {installed ? (
          <>
            <p className={styles.cliPath}>{t('prefs.ollamaInstalled')}</p>

            <div>
              <span>{t('prefs.ollamaModels')}</span>
              <ul>
                {models.length === 0 ? <li>{t('prefs.ollamaModelsEmpty')}</li> : null}
                {models.map((model) => (
                  <li key={model.name}>{model.name}</li>
                ))}
              </ul>
              <div className={styles.cliActions}>
                <input
                  className={controls.input}
                  value={pullTarget}
                  placeholder={t('prefs.ollamaPullPlaceholder')}
                  onChange={(event) => setPullTarget(event.target.value)}
                  spellCheck={false}
                />
                <button
                  type="button"
                  className={`${controls.btn} ${controls.btnPrimary}`}
                  disabled={!pullTarget.trim() || pullingModel !== null}
                  onClick={() => void pull()}
                >
                  {pullingModel
                    ? t('prefs.ollamaPulling', { model: pullingModel })
                    : t('prefs.ollamaPull')}
                </button>
              </div>
            </div>

            <div>
              <span>{t('prefs.ollamaInstances')}</span>
              <ul>
                {instances.length === 0 ? <li>{t('prefs.ollamaInstancesEmpty')}</li> : null}
                {instances.map((instance) => (
                  <li key={instance.id}>
                    {instance.model} — {t('prefs.ollamaInstancePort', { port: instance.port })}
                    <button
                      type="button"
                      className={`${controls.btn} ${controls.btnDanger}`}
                      onClick={() => void stopInstance(instance.id)}
                    >
                      {t('prefs.ollamaStopInstance')}
                    </button>
                  </li>
                ))}
              </ul>
              <div className={styles.cliActions}>
                <select
                  className={controls.input}
                  value={startModel}
                  onChange={(event) => setStartModel(event.target.value)}
                >
                  <option value="">{t('prefs.ollamaSelectModel')}</option>
                  {models.map((model) => (
                    <option key={model.name} value={model.name}>
                      {model.name}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className={`${controls.btn} ${controls.btnPrimary}`}
                  disabled={!startModel}
                  onClick={() => void startInstance()}
                >
                  {t('prefs.ollamaStartInstance')}
                </button>
              </div>
            </div>
          </>
        ) : null}

        {error ? <p className={styles.cliWarning}>{error}</p> : null}
      </div>
    </SettingsSection>
  )
}

function OptimizerSection() {
  const t = useT()
  const preferences = useProjectsStore((state) => state.preferences)
  const setPreferences = useProjectsStore((state) => state.setPreferences)
  const [cavemanInstalled, setCavemanInstalled] = useState<boolean | null>(null)
  const [rtkInstalled, setRtkInstalled] = useState<boolean | null>(null)
  const [headroomInstalled, setHeadroomInstalled] = useState<boolean | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refreshDetection = () => {
    void findCliLauncher('caveman').then((path) => setCavemanInstalled(path !== null))
    void findCliLauncher('rtk').then((path) => setRtkInstalled(path !== null))
    void findCliLauncher('headroom').then((path) => setHeadroomInstalled(path !== null))
  }

  useEffect(refreshDetection, [])

  const run = async (key: string, action: () => Promise<void>) => {
    setBusy(key)
    setError(null)
    try {
      await action()
      refreshDetection()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(null)
    }
  }

  const setWrapper = (wrapper: Preferences['optimizerWrapper']) => {
    setPreferences({ optimizerWrapper: wrapper })
  }

  return (
    <SettingsSection
      id="optimizer"
      title={t('prefs.optimizer')}
      description={t('prefs.optimizerDesc')}
    >
      <div className={styles.integrationFields}>
        <div className={styles.segmented}>
          <button
            type="button"
            className={preferences.optimizerWrapper === 'none' ? styles.segmentActive : undefined}
            onClick={() => setWrapper('none')}
          >
            {t('prefs.optimizerNone')}
          </button>
          <button
            type="button"
            className={
              preferences.optimizerWrapper === 'caveman' ? styles.segmentActive : undefined
            }
            onClick={() => setWrapper('caveman')}
          >
            Caveman
          </button>
          <button
            type="button"
            className={
              preferences.optimizerWrapper === 'headroom' ? styles.segmentActive : undefined
            }
            onClick={() => setWrapper('headroom')}
          >
            Headroom
          </button>
        </div>

        <div className={styles.cliActions}>
          <span>
            Caveman —{' '}
            {cavemanInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}
          </span>
          <button
            type="button"
            className={`${controls.btn} ${controls.btnPrimary}`}
            disabled={busy !== null}
            onClick={() => void run('caveman', optimizerInstallCaveman)}
          >
            {busy === 'caveman' ? t('prefs.optimizerInstalling') : t('prefs.optimizerInstall')}
          </button>
        </div>

        <div className={styles.cliActions}>
          <span>
            Headroom —{' '}
            {headroomInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}
          </span>
          <button
            type="button"
            className={`${controls.btn} ${controls.btnPrimary}`}
            disabled={busy !== null}
            onClick={() => void run('headroom', optimizerInstallHeadroom)}
          >
            {busy === 'headroom' ? t('prefs.optimizerInstalling') : t('prefs.optimizerInstall')}
          </button>
        </div>

        <div className={styles.cliActions}>
          <span>
            RTK — {rtkInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}
          </span>
          <button
            type="button"
            className={`${controls.btn} ${controls.btnPrimary}`}
            disabled={busy !== null}
            onClick={() => void run('rtk', optimizerInstallRtk)}
          >
            {busy === 'rtk' ? t('prefs.optimizerInstalling') : t('prefs.optimizerInstall')}
          </button>
          {rtkInstalled ? (
            <button
              type="button"
              className={controls.btn}
              disabled={busy !== null}
              onClick={() => void run('rtk-configure', optimizerConfigureRtk)}
            >
              {busy === 'rtk-configure'
                ? t('prefs.optimizerRtkConfiguring')
                : t('prefs.optimizerRtkConfigure')}
            </button>
          ) : null}
        </div>

        {error ? <p className={styles.cliWarning}>{error}</p> : null}
      </div>
    </SettingsSection>
  )
}

function BucketStatusBadge({ command }: { command: string }) {
  const t = useT()
  const [resolved, setResolved] = useState<boolean | null>(null)

  useEffect(() => {
    let disposed = false
    const trimmed = command.trim()
    if (!trimmed) {
      setResolved(null)
      return
    }
    void findCliLauncher(trimmed).then((path) => {
      if (!disposed) setResolved(path !== null)
    })
    return () => {
      disposed = true
    }
  }, [command])

  if (resolved === null) return null
  return (
    <span
      className={`${styles.bucketStatus} ${resolved ? styles.bucketStatusOk : styles.bucketStatusMissing}`}
    >
      {resolved ? t('prefs.orchestratorBucketFound') : t('prefs.orchestratorBucketMissing')}
    </span>
  )
}

function OrchestratorBucketsSection() {
  const t = useT()
  const preferences = useProjectsStore((state) => state.preferences)
  const setPreferences = useProjectsStore((state) => state.setPreferences)
  const buckets = preferences.orchestratorBuckets

  const updateBucket = (id: string, patch: Partial<OrchestratorBucketConfig>) => {
    setPreferences({
      orchestratorBuckets: buckets.map((bucket) =>
        bucket.id === id ? { ...bucket, ...patch } : bucket,
      ),
    })
  }

  const addBucket = () => {
    const bucket: OrchestratorBucketConfig = {
      id: nanoid(8),
      label: '',
      command: '',
      protocol: 'oneShot',
      args: [],
      modelFlag: '--model',
      model: '',
      fallback: '',
      env: '',
    }
    setPreferences({ orchestratorBuckets: [...buckets, bucket] })
  }

  const removeBucket = (id: string) => {
    setPreferences({
      orchestratorBuckets: buckets
        .filter((bucket) => bucket.id !== id)
        .map((bucket) => (bucket.fallback === id ? { ...bucket, fallback: '' } : bucket)),
    })
  }

  const fallbackOptions = (selfId: string) =>
    [
      { id: 'codex', label: 'Codex (default)' },
      { id: 'opencode', label: 'OpenCode (default)' },
      ...buckets.map((bucket) => ({
        id: bucket.id,
        label: bucket.label || bucket.command || bucket.id,
      })),
    ].filter((option) => option.id !== selfId)

  return (
    <SettingsSection
      id="orchestrator-buckets"
      title={t('prefs.orchestratorBuckets')}
      description={t('prefs.orchestratorBucketsDesc')}
    >
      <div className={styles.integrationFields}>
        <p>{t('prefs.orchestratorBucketsHint')}</p>

        <div className={styles.bucketList}>
          {buckets.map((bucket) => (
            <div key={bucket.id} className={styles.bucketCard}>
              <div className={styles.bucketRow}>
                <input
                  className={controls.input}
                  value={bucket.label}
                  placeholder={t('prefs.orchestratorBucketLabel')}
                  onChange={(event) => updateBucket(bucket.id, { label: event.target.value })}
                  spellCheck={false}
                />
                <input
                  className={controls.input}
                  value={bucket.command}
                  placeholder={t('prefs.orchestratorBucketCommand')}
                  onChange={(event) => updateBucket(bucket.id, { command: event.target.value })}
                  spellCheck={false}
                />
                <BucketStatusBadge command={bucket.command} />
                <button
                  type="button"
                  className={`${controls.btn} ${controls.btnDanger}`}
                  onClick={() => removeBucket(bucket.id)}
                >
                  {t('prefs.orchestratorBucketRemove')}
                </button>
              </div>

              <div className={styles.bucketRow}>
                <div className={styles.segmented}>
                  <button
                    type="button"
                    className={bucket.protocol === 'appServer' ? styles.segmentActive : undefined}
                    onClick={() => updateBucket(bucket.id, { protocol: 'appServer' })}
                  >
                    {t('prefs.orchestratorProtocolAppServer')}
                  </button>
                  <button
                    type="button"
                    className={bucket.protocol === 'oneShot' ? styles.segmentActive : undefined}
                    onClick={() => updateBucket(bucket.id, { protocol: 'oneShot' })}
                  >
                    {t('prefs.orchestratorProtocolOneShot')}
                  </button>
                </div>
                <input
                  className={controls.input}
                  value={bucket.model}
                  placeholder={t('prefs.orchestratorBucketModel')}
                  onChange={(event) => updateBucket(bucket.id, { model: event.target.value })}
                  spellCheck={false}
                />
                <select
                  className={controls.input}
                  value={bucket.fallback}
                  onChange={(event) => updateBucket(bucket.id, { fallback: event.target.value })}
                >
                  <option value="">{t('prefs.orchestratorBucketFallbackNone')}</option>
                  {fallbackOptions(bucket.id).map((option) => (
                    <option key={option.id} value={option.id}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </div>

              {bucket.protocol === 'oneShot' ? (
                <div className={styles.bucketRow}>
                  <input
                    className={controls.input}
                    value={bucket.args.join(' ')}
                    placeholder={t('prefs.orchestratorBucketArgs')}
                    onChange={(event) =>
                      updateBucket(bucket.id, {
                        args: event.target.value.split(/\s+/).filter(Boolean),
                      })
                    }
                    spellCheck={false}
                  />
                  <input
                    className={controls.input}
                    value={bucket.modelFlag}
                    placeholder={t('prefs.orchestratorBucketModelFlag')}
                    onChange={(event) => updateBucket(bucket.id, { modelFlag: event.target.value })}
                    spellCheck={false}
                  />
                </div>
              ) : null}

              <textarea
                className={`${controls.input} ${styles.bucketEnv}`}
                value={bucket.env}
                placeholder={t('prefs.orchestratorBucketEnvPlaceholder')}
                onChange={(event) => updateBucket(bucket.id, { env: event.target.value })}
                spellCheck={false}
                rows={2}
              />
            </div>
          ))}
        </div>

        <div className={styles.cliActions}>
          <button
            type="button"
            className={`${controls.btn} ${controls.btnPrimary}`}
            onClick={addBucket}
          >
            {t('prefs.orchestratorBucketAdd')}
          </button>
        </div>
      </div>
    </SettingsSection>
  )
}

export function IntegrationsPage() {
  const t = useT()
  const preferences = useProjectsStore((state) => state.preferences)
  const setPreferences = useProjectsStore((state) => state.setPreferences)
  return (
    <>
      <TerminalCommandSection />

      <OllamaSection />

      <OptimizerSection />

      <OrchestratorBucketsSection />

      <SettingsSection id="spotify" title={t('prefs.spotify')} description={t('prefs.spotifyDesc')}>
        <div className={styles.integrationFields}>
          <label>
            <span>Client ID</span>
            <input
              className={controls.input}
              value={preferences.spotifyClientId}
              onChange={(event) => setPreferences({ spotifyClientId: event.target.value })}
              spellCheck={false}
            />
          </label>
          <label>
            <span>Client Secret</span>
            <input
              className={controls.input}
              type="password"
              value={preferences.spotifyClientSecret}
              onChange={(event) => setPreferences({ spotifyClientSecret: event.target.value })}
              spellCheck={false}
            />
          </label>
          <p>
            {t('prefs.spotifyHint', {
              redirect: 'http://127.0.0.1:8888/callback',
              idEnv: 'SPOTIFY_CLIENT_ID',
              secretEnv: 'SPOTIFY_CLIENT_SECRET',
            })}
          </p>
        </div>
      </SettingsSection>

      <SettingsSection
        id="discord"
        title={t('prefs.discordPresence')}
        description={t('prefs.discordPresenceHint')}
      >
        <div className={styles.segmented}>
          <button
            type="button"
            className={preferences.discordRichPresenceEnabled ? styles.segmentActive : undefined}
            onClick={() => setPreferences({ discordRichPresenceEnabled: true })}
          >
            {t('prefs.discordPresenceEnabled')}
          </button>
          <button
            type="button"
            className={!preferences.discordRichPresenceEnabled ? styles.segmentActive : undefined}
            onClick={() => setPreferences({ discordRichPresenceEnabled: false })}
          >
            {t('prefs.discordPresenceDisabled')}
          </button>
        </div>
      </SettingsSection>

      <SettingsSection
        id="dictation"
        title={t('prefs.dictation')}
        description={t('prefs.dictationDesc')}
      >
        <div className={styles.segmented}>
          <button
            type="button"
            className={preferences.dictationEnabled ? styles.segmentActive : undefined}
            onClick={() => setPreferences({ dictationEnabled: true })}
          >
            {t('prefs.dictationOn')}
          </button>
          <button
            type="button"
            className={!preferences.dictationEnabled ? styles.segmentActive : undefined}
            onClick={() => setPreferences({ dictationEnabled: false })}
          >
            {t('prefs.dictationOff')}
          </button>
        </div>
        <p>{t('prefs.dictationHandyHint')}</p>
      </SettingsSection>
    </>
  )
}
