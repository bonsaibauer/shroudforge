import type { ModInfo } from './mod-types'
import { postRequest } from '../../transport/ui/send-host-command'
import { useEffect, useRef, useState } from 'react'
import type { CommandResult } from '../../notifications/ui/notification-types'
import { applySettingChange } from './apply-setting-presets'

function settingsValues(mod: ModInfo): Record<string, unknown> {
  return Object.fromEntries(
    mod.settings
      .filter((item) => item.key)
      .map((item) => [
        item.key,
        mod.settingValues[item.key] ??
          item.default ??
          (item.type === 'boolean' ? false : ''),
      ]),
  )
}

export function useModSettings(mod: ModInfo) {
  const [enabled, setLocalEnabled] = useState(mod.enabled)
  const initial = () => settingsValues(mod)
  const [values, setValues] = useState<Record<string, unknown>>(initial)
  const valuesRef = useRef(values)
  const revisionRef = useRef(mod.revision)
  const modIdRef = useRef(mod.id)
  const autoSaveTimer = useRef<number | undefined>(undefined)
  const pendingAutoSave = useRef(false)
  const latestMod = useRef(mod)
  latestMod.current = mod
  const requests = useRef(new Map<string, string>())
  const request = (
    command: 'save-mod-settings' | 'set-mod-enabled',
    payload: Record<string, unknown> & { modId: string },
  ) => {
    const requestId = postRequest(command, payload)
    requests.current.set(requestId, payload.modId)
  }
  useEffect(() => {
    const changedMod = modIdRef.current !== mod.id
    if (changedMod) {
      if (autoSaveTimer.current !== undefined) {
        window.clearTimeout(autoSaveTimer.current)
        autoSaveTimer.current = undefined
      }
      if (pendingAutoSave.current) {
        pendingAutoSave.current = false
        request('save-mod-settings', {
          modId: modIdRef.current,
          values: valuesRef.current,
          revision: revisionRef.current,
        })
      }
      modIdRef.current = mod.id
      valuesRef.current = initial()
      setValues(valuesRef.current)
    } else if (!pendingAutoSave.current) {
      valuesRef.current = initial()
      setValues(valuesRef.current)
    }
    revisionRef.current = mod.revision
    setLocalEnabled(mod.enabled)
  }, [mod.id, mod.revision])
  useEffect(
    () => () => {
      if (autoSaveTimer.current !== undefined)
        window.clearTimeout(autoSaveTimer.current)
      if (pendingAutoSave.current)
        request('save-mod-settings', {
          modId: modIdRef.current,
          values: valuesRef.current,
          revision: revisionRef.current,
        })
    },
    [],
  )
  useEffect(() => {
    const restoreFailedChange = (event: Event) => {
      const result = (event as CustomEvent<CommandResult>).detail
      const requestedModId = requests.current.get(result.requestId)
      if (!requestedModId) return
      requests.current.delete(result.requestId)
      const current = latestMod.current
      if (result.success || requestedModId !== current.id || requestedModId !== modIdRef.current) return
      if (autoSaveTimer.current !== undefined) {
        window.clearTimeout(autoSaveTimer.current)
        autoSaveTimer.current = undefined
      }
      pendingAutoSave.current = false
      valuesRef.current = settingsValues(current)
      revisionRef.current = current.revision
      setValues(valuesRef.current)
      setLocalEnabled(current.enabled)
    }
    window.addEventListener('shroudforge-command-result', restoreFailedChange)
    return () => window.removeEventListener('shroudforge-command-result', restoreFailedChange)
  }, [])
  const save = () => {
    if (autoSaveTimer.current !== undefined) {
      window.clearTimeout(autoSaveTimer.current)
      autoSaveTimer.current = undefined
    }
    pendingAutoSave.current = false
    request('save-mod-settings', {
      modId: mod.id,
      values: valuesRef.current,
      revision: revisionRef.current,
    })
  }
  const updateSetting = (key: string, value: unknown) => {
    const next = applySettingChange(
      valuesRef.current,
      latestMod.current.presets,
      key,
      value,
    )
    valuesRef.current = next
    setValues(next)
    pendingAutoSave.current = true
    if (autoSaveTimer.current !== undefined)
      window.clearTimeout(autoSaveTimer.current)
    autoSaveTimer.current = window.setTimeout(() => {
      autoSaveTimer.current = undefined
      if (!pendingAutoSave.current) return
      pendingAutoSave.current = false
      request('save-mod-settings', {
        modId: modIdRef.current,
        values: valuesRef.current,
        revision: revisionRef.current,
      })
    }, 350)
  }
  const setEnabled = (next: boolean) => {
    setLocalEnabled(next)
    request('set-mod-enabled', {
      modId: mod.id,
      enabled: next,
      revision: revisionRef.current,
    })
  }
  return { enabled, values, save, updateSetting, setEnabled }
}
