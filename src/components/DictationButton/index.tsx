import { Mic, MicOff } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

import { useT } from '../../lib/i18n'
import { writePty } from '../../lib/tauri'
import { useProjectsStore } from '../../stores/projectsStore'
import { useUiStore } from '../../stores/uiStore'
import styles from './DictationButton.module.css'

/** Transcribe speech into the active terminal when dictation is enabled. */

// Web Speech API types are not included in the default DOM library.
type SpeechRecognitionLike = {
  lang: string
  continuous: boolean
  interimResults: boolean
  start(): void
  stop(): void
  onresult: ((event: SpeechResultEvent) => void) | null
  onend: (() => void) | null
  onerror: (() => void) | null
}
type SpeechResultEvent = {
  resultIndex: number
  results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }>
}

function getRecognitionCtor(): (new () => SpeechRecognitionLike) | null {
  const w = window as unknown as {
    SpeechRecognition?: new () => SpeechRecognitionLike
    webkitSpeechRecognition?: new () => SpeechRecognitionLike
  }
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null
}

/** Active terminal PTY, if any. */
function activePtyId(): string | null {
  const target = useUiStore.getState().activeTerminal
  if (!target) return null
  const project = useProjectsStore.getState().projects.find((p) => p.id === target.projectId)
  const terminal = project?.terminals.find((t) => t.id === target.terminalId)
  const tab = terminal?.tabs.find((t) => t.id === terminal.activeTabId) ?? terminal?.tabs[0]
  return tab?.ptyId ?? null
}

export function DictationButton() {
  const t = useT()
  const enabled = useProjectsStore((s) => s.preferences.dictationEnabled)
  const language = useProjectsStore((s) => s.preferences.language)
  const [listening, setListening] = useState(false)
  const recRef = useRef<SpeechRecognitionLike | null>(null)
  const supported = getRecognitionCtor() !== null

  const stop = useCallback(() => {
    try {
      recRef.current?.stop()
    } catch {
      /* Already stopped. */
    }
    recRef.current = null
    setListening(false)
  }, [])

  const start = useCallback(() => {
    const Ctor = getRecognitionCtor()
    if (!Ctor) return
    const rec = new Ctor()
    rec.lang = language === 'pt-BR' ? 'pt-BR' : 'en-US'
    rec.continuous = true
    rec.interimResults = false
    rec.onresult = (event) => {
      for (let i = event.resultIndex; i < event.results.length; i++) {
        const result = event.results[i]
        if (!result.isFinal) continue
        const text = String(result[0]?.transcript ?? '').trim()
        const ptyId = activePtyId()
        if (text && ptyId) void writePty(ptyId, `${text} `)
      }
    }
    rec.onend = () => {
      recRef.current = null
      setListening(false)
    }
    rec.onerror = () => {
      recRef.current = null
      setListening(false)
    }
    recRef.current = rec
    setListening(true)
    try {
      rec.start()
    } catch {
      recRef.current = null
      setListening(false)
    }
  }, [language])

  // Clean up on unmount or disable.
  useEffect(
    () => () => {
      try {
        recRef.current?.stop()
      } catch {
        /* noop */
      }
      recRef.current = null
    },
    [],
  )

  if (!enabled) return null

  return (
    <button
      type="button"
      className={`${styles.btn} ${listening ? styles.listening : ''}`}
      onClick={() => (listening ? stop() : start())}
      disabled={!supported}
      title={
        !supported
          ? t('dictation.unsupported')
          : listening
            ? t('dictation.stop')
            : t('dictation.start')
      }
      aria-label={t('dictation.label')}
      aria-pressed={listening}
    >
      {listening ? <Mic size={18} /> : <MicOff size={18} />}
    </button>
  )
}
