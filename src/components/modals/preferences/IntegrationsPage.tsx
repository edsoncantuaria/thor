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
import type { Preferences } from '../../../lib/types'
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
          <code>{'alethe\nalethe .\nalethe ~/meu-projeto'}</code>
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
                  {pullingModel ? t('prefs.ollamaPulling', { model: pullingModel }) : t('prefs.ollamaPull')}
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
    <SettingsSection id="optimizer" title={t('prefs.optimizer')} description={t('prefs.optimizerDesc')}>
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
            className={preferences.optimizerWrapper === 'caveman' ? styles.segmentActive : undefined}
            onClick={() => setWrapper('caveman')}
          >
            Caveman
          </button>
          <button
            type="button"
            className={preferences.optimizerWrapper === 'headroom' ? styles.segmentActive : undefined}
            onClick={() => setWrapper('headroom')}
          >
            Headroom
          </button>
        </div>

        <div className={styles.cliActions}>
          <span>Caveman — {cavemanInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}</span>
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
          <span>Headroom — {headroomInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}</span>
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
          <span>RTK — {rtkInstalled ? t('prefs.optimizerInstalled') : t('prefs.optimizerNotInstalled')}</span>
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
              {busy === 'rtk-configure' ? t('prefs.optimizerRtkConfiguring') : t('prefs.optimizerRtkConfigure')}
            </button>
          ) : null}
        </div>

        {error ? <p className={styles.cliWarning}>{error}</p> : null}
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
