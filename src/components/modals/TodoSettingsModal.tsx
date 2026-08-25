import { Folder, RotateCcw } from 'lucide-react'
import { useEffect, useState } from 'react'

import { pickDirectory } from '../../lib/dialog'
import { useT } from '../../lib/i18n'
import { ensureTodoTemplate } from '../../lib/tauri'
import { useProjectsStore } from '../../stores/projectsStore'
import { useUiStore } from '../../stores/uiStore'
import controls from './controls.module.css'
import { Modal } from './Modal'

export function TodoSettingsModal() {
  const t = useT()
  const open = useUiStore((state) => state.openModal === 'todoSettings')
  const closeModal = useUiStore((state) => state.closeModal)
  const savedPath = useProjectsStore((state) => state.preferences.todoStoragePath)
  const setPreferences = useProjectsStore((state) => state.setPreferences)
  const resetTodosToDefault = useProjectsStore((state) => state.resetTodosToDefault)
  const [path, setPath] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (open) setPath(savedPath)
  }, [open, savedPath])

  const browse = async () => {
    const selected = await pickDirectory({ defaultPath: path || savedPath || undefined })
    if (selected) setPath(selected)
  }

  const save = async () => {
    const finalPath = path.trim()
    setSaving(true)
    try {
      if (finalPath) {
        await ensureTodoTemplate(finalPath)
      }
      setPreferences({ todoStoragePath: finalPath })
      closeModal()
    } catch (error) {
      window.alert(t('todo.templateError', { message: String(error) }))
    } finally {
      setSaving(false)
    }
  }

  const resetDefault = () => {
    if (!window.confirm(t('todo.resetDefaultConfirm'))) return
    resetTodosToDefault()
    closeModal()
  }

  return (
    <Modal
      open={open}
      onClose={closeModal}
      title={t('todo.settingsTitle')}
      width={520}
      footer={
        <>
          <button type="button" className={controls.btn} onClick={closeModal}>
            {t('common.cancel')}
          </button>
          <button
            type="button"
            className={`${controls.btn} ${controls.btnPrimary}`}
            onClick={() => void save()}
            disabled={saving}
          >
            {t('common.save')}
          </button>
        </>
      }
    >
      <div className={controls.field}>
        <label className={controls.label}>{t('todo.pathLabel')}</label>
        <div className={controls.cwdRow}>
          <input
            className={controls.input}
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder={t('todo.pathPlaceholder')}
          />
          <button
            type="button"
            className={controls.btn}
            onClick={browse}
            title={t('todo.choosePath')}
            aria-label={t('todo.choosePath')}
          >
            <Folder size={14} />
          </button>
          <button
            type="button"
            className={controls.btn}
            onClick={() => setPath('')}
            title={t('todo.clearPath')}
            aria-label={t('todo.clearPath')}
          >
            <RotateCcw size={14} />
          </button>
        </div>
      </div>
      <div className={controls.field}>
        <label className={controls.label}>{t('todo.defaultLabel')}</label>
        <button type="button" className={controls.btn} onClick={resetDefault}>
          {t('todo.resetDefault')}
        </button>
      </div>
    </Modal>
  )
}
