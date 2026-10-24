import { useCallback, useEffect, useRef, useState } from 'react'
import { useI18n } from '../../ui-resources/i18n'
import { postRequest } from '../../transport/ui/send-host-command'

const requestedLabels = new Set<string>()
let labelRequestTimer: number | undefined
let cancelActiveRecording: (() => void) | undefined
type InputDevice = 'keyboard' | 'mouse'
const inputDevices: InputDevice[] = ['keyboard', 'mouse']

function requestKeybindLabel(value: string) {
  if (!value) return
  requestedLabels.add(value)
  if (labelRequestTimer !== undefined) return
  labelRequestTimer = window.setTimeout(() => {
    labelRequestTimer = undefined
    const values = [...requestedLabels]
    requestedLabels.clear()
    for (let index = 0; index < values.length; index += 256) {
      postRequest('keybind-labels', { values: values.slice(index, index + 256) })
    }
  }, 0)
}

export function keybindText(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function deviceBinding(value: string, device: InputDevice): string {
  const trigger = device === 'keyboard' ? /(?:^|\+)SDL:\d+$/ : /(?:^|\+)Mouse:\d+$/
  return value.split('|').find((alternative) => trigger.test(alternative)) || ''
}

function replaceDeviceBinding(value: string, device: InputDevice, binding: string): string {
  return inputDevices.map((current) => current === device ? binding : deviceBinding(value, current))
    .filter(Boolean).join('|')
}

export function KeybindInput({
  value,
  label,
  onChange,
  onReset,
  disabled = false,
}: {
  value: unknown
  label: string
  onChange(value: string): void
  onReset?(): void
  disabled?: boolean
}) {
  const { t } = useI18n()
  const [recording, setRecording] = useState<InputDevice | null>(null)
  const [nativeLabels, setNativeLabels] = useState<Record<string, string>>({})
  const editor = useRef<HTMLDivElement>(null)
  const recordingRef = useRef(recording)
  recordingRef.current = recording
  const captureRequestId = useRef('')
  const pointerCancelled = useRef<InputDevice | null>(null)
  const suppressKeyboard = useRef(false)
  const suppressMouseClick = useRef(false)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  const text = keybindText(value)
  const valueRef = useRef(text)
  valueRef.current = text
  const cancelRecording = useCallback(() => {
    const requestId = captureRequestId.current
    captureRequestId.current = ''
    if (requestId) postRequest('keybind-capture', { enabled: false, id: requestId })
    suppressKeyboard.current = false
    suppressMouseClick.current = false
    setRecording(null)
  }, [])
  useEffect(() => {
    if (recording !== 'mouse') return
    let releaseTimer: number | undefined
    const events = ['pointerdown', 'pointerup', 'click', 'auxclick', 'contextmenu']
    const remove = () => {
      if (releaseTimer !== undefined) window.clearTimeout(releaseTimer)
      events.forEach((name) => window.removeEventListener(name, blockRecordingClick, true))
    }
    const blockRecordingClick = (event: Event) => {
      const mouseRecording = recordingRef.current === 'mouse'
      const button = event.target instanceof Element ? event.target.closest('button') : null
      const ownButton = button && editor.current?.contains(button)
      const captureSurface = ownButton && button.dataset.captureDevice === 'mouse' && mouseRecording
      if (event.type === 'pointerdown') {
        suppressMouseClick.current = false
        if (!mouseRecording) {
          remove()
          return
        }
        if (ownButton && !captureSurface) return
        suppressMouseClick.current = true
      }
      if (!mouseRecording && !suppressMouseClick.current) return
      if (ownButton && !captureSurface && !suppressMouseClick.current) return
      event.preventDefault()
      event.stopPropagation()
      if (event.type === 'click' || event.type === 'auxclick') {
        suppressMouseClick.current = false
        if (!mouseRecording) remove()
      }
    }
    events.forEach((name) => window.addEventListener(name, blockRecordingClick, true))
    return () => {
      if (recordingRef.current === 'mouse' || !suppressMouseClick.current) remove()
      else releaseTimer = window.setTimeout(remove, 250)
    }
  }, [recording])
  useEffect(() => {
    const updateLabel = (event: Event) => {
      const detail = (event as CustomEvent<{ labels?: Record<string, string> }>).detail
      const labels: Record<string, string> = {}
      inputDevices.forEach((device) => {
        const binding = deviceBinding(text, device)
        const nativeLabel = detail?.labels?.[binding]
        if (typeof nativeLabel === 'string') labels[binding] = nativeLabel
      })
      if (Object.keys(labels).length) setNativeLabels((current) => ({ ...current, ...labels }))
    }
    const refresh = () => inputDevices.forEach((device) => requestKeybindLabel(deviceBinding(text, device)))
    window.addEventListener('shroudforge-keybind-labels', updateLabel)
    window.addEventListener('shroudforge-keybind-labels-changed', refresh)
    window.addEventListener('focus', refresh)
    refresh()
    return () => {
      window.removeEventListener('shroudforge-keybind-labels', updateLabel)
      window.removeEventListener('shroudforge-keybind-labels-changed', refresh)
      window.removeEventListener('focus', refresh)
    }
  }, [text])
  useEffect(() => {
    if (!recording) return
    cancelActiveRecording?.()
    cancelActiveRecording = cancelRecording
    let requestId = ''
    const capture = (event: Event) => {
      const detail = (event as CustomEvent<{
        requestId: string
        value?: string
        label?: string
        error?: string
      }>).detail
      if (!detail || detail.requestId !== requestId || captureRequestId.current !== requestId) return
      if (detail.error) {
        cancelRecording()
        return
      }
      if (typeof detail.value !== 'string' || typeof detail.label !== 'string') return
      if (!deviceBinding(detail.value, recording)) return
      // Escape opens the game's pause menu and closes its windows: a shortcut
      // on it would fire at the same time. Pressing it cancels the recording
      // and keeps the previous value, with or without modifiers.
      if (/(?:^|\+)SDL:41$/.test(deviceBinding(detail.value, 'keyboard'))) {
        cancelRecording()
        return
      }
      const capturedValue = detail.value
      const capturedLabel = detail.label
      setNativeLabels((current) => ({ ...current, [capturedValue]: capturedLabel }))
      setRecording(null)
      onChangeRef.current(replaceDeviceBinding(valueRef.current, recording, capturedValue))
    }
    window.addEventListener('shroudforge-keybind-captured', capture)
    window.addEventListener('blur', cancelRecording)
    requestId = postRequest('keybind-capture', { enabled: true, device: recording })
    captureRequestId.current = requestId
    return () => {
      window.removeEventListener('shroudforge-keybind-captured', capture)
      window.removeEventListener('blur', cancelRecording)
      if (captureRequestId.current === requestId) captureRequestId.current = ''
      if (cancelActiveRecording === cancelRecording) cancelActiveRecording = undefined
      postRequest('keybind-capture', { enabled: false, id: requestId })
    }
  }, [recording, cancelRecording])
  useEffect(() => {
    if (disabled) cancelRecording()
  }, [disabled, cancelRecording])
  return (
    <div
      ref={editor}
      className="keybind-editor"
      onPointerDownCapture={(event) => {
        pointerCancelled.current = null
        if (!recording) return
        const button = (event.target as Element).closest<HTMLButtonElement>('button')
        if (!button) return
        if (recording === 'mouse' && button.dataset.captureDevice === 'mouse') {
          event.preventDefault()
          event.stopPropagation()
          return
        }
        pointerCancelled.current = button.dataset.captureDevice === recording ? recording : null
        cancelRecording()
      }}
      onKeyDownCapture={(event) => {
        if (recording || suppressKeyboard.current) {
          event.preventDefault()
          event.stopPropagation()
        }
      }}
      onKeyUpCapture={(event) => {
        if (recording || suppressKeyboard.current) {
          event.preventDefault()
          event.stopPropagation()
          suppressKeyboard.current = false
        }
      }}
      onBlur={(event) => {
        if (recording === 'keyboard' && !event.currentTarget.contains(event.relatedTarget)) cancelRecording()
      }}
    >
      {inputDevices.map((device) => {
        const binding = deviceBinding(text, device)
        const display = nativeLabels[binding] || binding
        const active = recording === device
        const action = `${label} (${t(`hotkeys.${device}`)})`
        return <div className="keybind-device" key={device}>
          <span className="keybind-device-label">{t(`hotkeys.${device}`)}</span>
          <button
            type="button"
            className="button secondary keybind-input"
            data-capture-device={device}
            disabled={disabled}
            aria-label={t('hotkeys.assignLabel', { action })}
            aria-pressed={active}
            title={`${t(active ? 'hotkeys.recordingHint' : 'hotkeys.assignHint')}${display ? ` (${display})` : ''}`}
            onClick={() => {
              if (pointerCancelled.current === device) {
                pointerCancelled.current = null
                return
              }
              if (active && device === 'mouse') return
              if (active) cancelRecording()
              else {
                suppressKeyboard.current = device === 'keyboard'
                setRecording(device)
              }
            }}
          >
            {active ? t('hotkeys.recording') : display || t('hotkeys.unassigned')}
          </button>
          {active ? (
            <button key="cancel" type="button" className="button ghost" onClick={cancelRecording}>
              {t('common.cancel')}
            </button>
          ) : (
            <button
              key="clear"
              type="button"
              className="button ghost"
              disabled={disabled || !binding}
              aria-label={t('hotkeys.clearLabel', { action })}
              onClick={() => onChange(replaceDeviceBinding(text, device, ''))}
            >
              {t('hotkeys.clear')}
            </button>
          )}
        </div>
      })}
      {!recording && onReset && (
        <button
          type="button"
          className="button ghost keybind-reset"
          disabled={disabled}
          aria-label={t('hotkeys.resetLabel', { action: label })}
          onClick={onReset}
        >
          {t('hotkeys.reset')}
        </button>
      )}
    </div>
  )
}
