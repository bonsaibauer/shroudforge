import { useEffect, useRef, useState } from 'react'
import { TranslationKey, useI18n } from '../../ui-resources/i18n'
import { EcosystemBadge } from '../../ui-resources/show-ecosystem-badge'
import type { CommandResult } from './notification-types'

type ToastMessage = {
  message: string
  level: CommandResult['level']
  ecosystem?: 'EML' | 'SF'
}

export function useCommandToasts() {
  const { t } = useI18n()
  const [message, setMessage] = useState<ToastMessage | null>(null)
  const tRef = useRef(t)
  tRef.current = t
  const toastTimer = useRef<number | undefined>(undefined)
  const settingsBatchTimer = useRef<number | undefined>(undefined)
  const settingsBatchCount = useRef(0)
  const notifyCommandResult = (result: CommandResult) => {
    let message = result.message
    if (result.success && result.batchKey === 'settings') {
      settingsBatchCount.current += 1
      message =
        settingsBatchCount.current === 1
          ? tRef.current('settings.settingSaved')
          : tRef.current('settings.settingsSaved', {
              count: settingsBatchCount.current,
            })
      if (settingsBatchTimer.current !== undefined)
        window.clearTimeout(settingsBatchTimer.current)
      settingsBatchTimer.current = window.setTimeout(() => {
        settingsBatchCount.current = 0
        settingsBatchTimer.current = undefined
      }, 900)
    } else {
      settingsBatchCount.current = 0
      if (settingsBatchTimer.current !== undefined) {
        window.clearTimeout(settingsBatchTimer.current)
        settingsBatchTimer.current = undefined
      }
      if (result.success)
        message = tRef.current(result.message as TranslationKey)
      else if (result.level === 'warn' && result.message === 'settings.moduleUnavailable')
        message = tRef.current('settings.moduleUnavailable')
    }
    setMessage({
      message,
      level: result.level,
      ecosystem: result.ecosystem || undefined,
    })
    if (toastTimer.current !== undefined)
      window.clearTimeout(toastTimer.current)
    toastTimer.current = window.setTimeout(
      () => {
        setMessage(null)
        toastTimer.current = undefined
      },
      result.level === 'error' ? 8000 : 4500,
    )
  }
  const dismissCommandToast = () => {
    if (toastTimer.current !== undefined) {
      window.clearTimeout(toastTimer.current)
      toastTimer.current = undefined
    }
    setMessage(null)
  }
  return { message, notifyCommandResult, dismissCommandToast }
}

export function Toasts({
  errors,
  message,
  onDismissMessage,
}: {
  errors?: string[]
  message: ToastMessage | null
  onDismissMessage(): void
}) {
  const { t } = useI18n()
  const errorsKey = JSON.stringify(errors ?? [])
  const [dismissedErrorsKey, setDismissedErrorsKey] = useState<string | null>(null)
  useEffect(() => {
    setDismissedErrorsKey(null)
  }, [errorsKey])
  const showErrors = Boolean(errors?.length) && dismissedErrorsKey !== errorsKey
  return showErrors || Boolean(message) ? (
    <div className="toast-stack" aria-live="polite">
      {showErrors && (
        <div role="alert" className="settings-save-message persistent error">
          <div className="toast-message-label">
            <span className="state-pill neutral">ERROR</span>
          </div>
          <div className="toast-message-content">
            <strong>{t('settings.configuration.title')}</strong>
            {errors?.map((error, index) => (
              <p key={error || index}>{error}</p>
            ))}
          </div>
          <button
            className="window-control window-close toast-close"
            onClick={() => setDismissedErrorsKey(errorsKey)}
            aria-label={t('common.dismiss')}
            title={t('common.dismiss')}
          >
            ×
          </button>
        </div>
      )}
      {message && (
        <div
          role={message.level === 'error' ? 'alert' : 'status'}
          className={`settings-save-message ${message.level === 'error' ? 'error' : message.level === 'warn' ? 'warning' : 'success'}`}
        >
          <div className="toast-message-label">
            <EcosystemBadge ecosystem={message.ecosystem} />
            <span className="state-pill neutral">{message.level.toUpperCase()}</span>
          </div>
          <div className="toast-message-content">
            <div className="toast-message-text">{message.message}</div>
          </div>
          <button
            className="window-control window-close toast-close"
            onClick={onDismissMessage}
            aria-label={t('common.dismiss')}
            title={t('common.dismiss')}
          >
            ×
          </button>
        </div>
      )}
    </div>
  ) : null
}
